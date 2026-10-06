// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use std::time::Duration;

use bytes::Bytes;
use dimidiumlabs_server::service::{
    AllowedRedirects, HostPattern, OutboundUriLayer, ResponseBodyDeadlineLayer,
    ResponseBodyLimitLayer, SafeRedirects, redirect::FollowRedirect,
};
use http_body_util::{BodyExt, Empty};
use hyper::{Request, http};
use hyper_tls::HttpsConnector;
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use tower::{ServiceBuilder, ServiceExt};
use url::Url;

use repos::{BackendError, BackendNetwork};

#[derive(Clone)]
pub struct DownloadRequest {
    pub url: Url,
}

#[derive(Clone)]
pub struct File {
    pub bytes: Bytes,
}

const MAX_DOWNLOAD_BYTES: usize = 512 * 1024 * 1024;
const RESPONSE_HEADER_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_BODY_DEADLINE: Duration = Duration::from_secs(5 * 60);
const MAX_REDIRECTS: usize = 5;
const TRUSTED_REDIRECT_HOSTS: &[&str] = &["dl.google.com"];

type HttpsClient = Client<HttpsConnector<HttpConnector>, Empty<Bytes>>;

pub struct ProxyService {
    client: HttpsClient,
}

impl ProxyService {
    pub fn new() -> Self {
        let https = HttpsConnector::new();
        let client = Client::builder(TokioExecutor::new()).build(https);
        Self { client }
    }

    pub async fn fetch(&self, request: DownloadRequest) -> Result<File, http::StatusCode> {
        let uri = request
            .url
            .as_str()
            .parse::<http::Uri>()
            .map_err(|_| http::StatusCode::BAD_GATEWAY)?;
        let allowed = outbound_policy(&uri)?;
        let redirects = FollowRedirect::with_policy(
            self.client.clone(),
            SafeRedirects::new(MAX_REDIRECTS, allowed.clone()),
        );
        let client = ServiceBuilder::new()
            .layer(OutboundUriLayer::new(allowed))
            .layer(tower::timeout::TimeoutLayer::new(RESPONSE_HEADER_TIMEOUT))
            .layer(ResponseBodyDeadlineLayer::new(RESPONSE_BODY_DEADLINE))
            .layer(ResponseBodyLimitLayer::new(MAX_DOWNLOAD_BYTES))
            .service(redirects);
        let request = Request::builder()
            .method(http::Method::GET)
            .uri(uri)
            .header(http::header::USER_AGENT, "pkg.earth/0.1")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let response = client
            .oneshot(request)
            .await
            .map_err(|_| http::StatusCode::GATEWAY_TIMEOUT)?;

        let (parts, body) = response.into_parts();
        let status = parts.status;
        if !status.is_success() {
            return Err(status);
        }
        if parts
            .headers
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length > MAX_DOWNLOAD_BYTES as u64)
        {
            return Err(http::StatusCode::BAD_GATEWAY);
        }

        let bytes = body
            .collect()
            .await
            .map_err(|_| http::StatusCode::GATEWAY_TIMEOUT)?
            .to_bytes();

        Ok(File { bytes })
    }
}

fn outbound_policy(uri: &http::Uri) -> Result<AllowedRedirects, http::StatusCode> {
    let destination = uri
        .authority()
        .ok_or(http::StatusCode::BAD_GATEWAY)
        .and_then(|authority| {
            HostPattern::new(authority.as_str()).map_err(|_| http::StatusCode::BAD_GATEWAY)
        })?;
    let redirects = TRUSTED_REDIRECT_HOSTS
        .iter()
        .map(|host| HostPattern::new(host).expect("trusted redirect host is valid"));
    Ok(AllowedRedirects::https_only(
        std::iter::once(destination).chain(redirects),
    ))
}

#[async_trait::async_trait]
impl BackendNetwork for ProxyService {
    async fn http_get(&self, url: &url::Url) -> Result<bytes::Bytes, BackendError> {
        self.fetch(DownloadRequest { url: url.clone() })
            .await
            .map(|f| f.bytes)
            .map_err(|e| BackendError::Network(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_policy_supports_go_downloads_without_open_redirects() {
        let policy = outbound_policy(
            &"https://go.dev/dl/go1.25.6.linux-amd64.tar.gz"
                .parse()
                .unwrap(),
        )
        .unwrap();
        assert!(
            policy.allows(
                &"https://dl.google.com/go/go1.25.6.linux-amd64.tar.gz"
                    .parse()
                    .unwrap()
            )
        );
        assert!(!policy.allows(&"https://attacker.example/archive".parse().unwrap()));
        assert!(
            !policy.allows(
                &"http://dl.google.com/go/go1.25.6.linux-amd64.tar.gz"
                    .parse()
                    .unwrap()
            )
        );
    }
}
