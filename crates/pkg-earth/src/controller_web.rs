// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use std::sync::{Arc, LazyLock};

use axum::extract;
use dimidiumlabs_server::{
    HtmlCompressionPredicate, assets_router,
    service::{
        HtmlLayer,
        compression::{CompressionLayer, CompressionLevel},
    },
};
use dimidiumlabs_ui::{AssetsCatalog, Document, FOUNDATION};
use maud::{Markup, Render};
use serde::Deserialize;
use tracing::error;

use repos::{GoBackend, ZigBackend};

use crate::ui::{
    APPLICATION,
    pages::{
        GoRelease, GoReleaseFile, IndexPage, LicenseEntry as PageLicenseEntry,
        LicenseOverview as PageLicenseOverview, LicenseUse, LicensesPage, ZigRelease,
        ZigReleaseFile,
    },
};

const DYNAMIC_COMPRESSION_MIN_BYTES: u16 = 128;
const DYNAMIC_COMPRESSION_LEVEL: CompressionLevel = CompressionLevel::Precise(5);

static ASSETS: LazyLock<Arc<AssetsCatalog>> = LazyLock::new(|| {
    Arc::new(
        AssetsCatalog::new()
            .with(FOUNDATION)
            .expect("foundation assets are valid")
            .with(APPLICATION)
            .expect("pkg.earth assets are valid and unique"),
    )
});

#[derive(Deserialize)]
struct LicenseList {
    overview: Vec<LicenseOverview>,
    licenses: Vec<LicenseEntry>,
}

#[derive(Deserialize)]
struct LicenseOverview {
    id: String,
    name: String,
    count: usize,
}

#[derive(Deserialize)]
struct LicenseEntry {
    id: String,
    name: String,
    text: String,
    first_of_kind: bool,
    used_by: Vec<LicenseUsedBy>,
}

#[derive(Deserialize)]
struct LicenseUsedBy {
    #[serde(rename = "crate")]
    crate_: LicenseCrate,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct LicenseCrate {
    name: String,
    version: String,
    repository: Option<String>,
}

static LICENSES: LazyLock<LicenseList> = LazyLock::new(|| {
    let json = include_str!(concat!(env!("OUT_DIR"), "/licenses.json"));
    serde_json::from_str(json).expect("failed to parse licenses.json")
});

/// Handles html pages rendering and static files
pub struct WebController {
    zig: Option<Arc<ZigBackend>>,
    go: Option<Arc<GoBackend>>,
}

impl WebController {
    pub fn new(zig: Option<Arc<ZigBackend>>, go: Option<Arc<GoBackend>>) -> Self {
        Self { zig, go }
    }
}

impl WebController {
    pub fn router(self: Arc<Self>) -> axum::Router {
        let assets = assets_router::<Arc<Self>>(Arc::clone(&ASSETS));

        let pages = axum::Router::new()
            .route("/", axum::routing::get(Self::index))
            .route("/about/licenses", axum::routing::get(Self::licenses))
            .layer(HtmlLayer::new(&ASSETS).with_negotiated_compression())
            .layer(
                CompressionLayer::new()
                    .quality(DYNAMIC_COMPRESSION_LEVEL)
                    .compress_when(HtmlCompressionPredicate::new(DYNAMIC_COMPRESSION_MIN_BYTES)),
            );

        pages.merge(assets).with_state(self)
    }

    async fn index(extract::State(ctrl): extract::State<Arc<Self>>) -> Markup {
        let zig = if let Some(ref backend) = ctrl.zig {
            match backend.get_releases().await {
                Ok(releases) => releases
                    .into_iter()
                    .rev()
                    .map(|release| ZigRelease {
                        version: release.version,
                        date: release.meta.date,
                        docs: release.meta.docs,
                        std_docs: release.meta.std_docs,
                        notes: release.meta.notes,
                        files: release
                            .files
                            .into_iter()
                            .map(|file| ZigReleaseFile {
                                filename: file.filename,
                                target: file.meta.target,
                            })
                            .collect(),
                    })
                    .collect(),
                Err(e) => {
                    error!("failed to get zig versions: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        let go = if let Some(ref backend) = ctrl.go {
            match backend.get_releases().await {
                Ok(releases) => releases
                    .into_iter()
                    .rev()
                    .map(|release| GoRelease {
                        version: release.version,
                        stable: release.meta.stable,
                        files: release
                            .files
                            .into_iter()
                            .map(|file| GoReleaseFile {
                                filename: file.filename,
                            })
                            .collect(),
                    })
                    .collect(),
                Err(e) => {
                    error!("failed to get go versions: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        Document::new(
            "pkg.earth — tiny & opinionated packages mirror",
            IndexPage { zig, go }.render(),
            &ASSETS,
        )
        .with_manifest()
        .with_svg_icon()
        .with_apple_touch_icon()
        .render()
    }

    async fn licenses() -> Markup {
        let data = &*LICENSES;
        let overview = data
            .overview
            .iter()
            .map(|entry| PageLicenseOverview {
                id: entry.id.clone(),
                name: entry.name.clone(),
                count: entry.count,
            })
            .collect();
        let licenses = data
            .licenses
            .iter()
            .map(|entry| PageLicenseEntry {
                id: entry.id.clone(),
                name: entry.name.clone(),
                text: entry.text.clone(),
                first_of_kind: entry.first_of_kind,
                used_by: entry
                    .used_by
                    .iter()
                    .map(|usage| LicenseUse {
                        name: usage.crate_.name.clone(),
                        version: usage.crate_.version.clone(),
                        repository: usage.crate_.repository.clone(),
                    })
                    .collect(),
            })
            .collect();
        Document::new(
            "Third Party Licenses",
            LicensesPage {
                project_license: include_str!("../../../LICENSE"),
                overview,
                licenses,
            }
            .render(),
            &ASSETS,
        )
        .with_manifest()
        .with_svg_icon()
        .with_apple_touch_icon()
        .render()
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode, header},
    };
    use dimidiumlabs_ui::ASSET_PREFIX;
    use tower::ServiceExt;

    use super::*;

    #[test]
    fn application_catalog_uses_generated_logical_and_fingerprinted_names() {
        let stylesheet = ASSETS
            .lookup("application.css")
            .expect("application stylesheet");
        let stylesheet =
            std::str::from_utf8(stylesheet.asset().bytes()).expect("UTF-8 application stylesheet");
        assert!(stylesheet.contains(":root{"), "missing application theme");
        for name in [
            "favicon.ico",
            "favicon.svg",
            "apple-touch-icon.png",
            "icon-192.png",
            "icon-512.png",
            "manifest.webmanifest",
            "robots.txt",
        ] {
            let asset = ASSETS
                .lookup(name)
                .expect("registered static asset")
                .asset();
            assert!(asset.integrity().starts_with("sha384-"));
            assert_eq!(asset.name(), asset.fingerprinted_name());
            assert_eq!(asset.cache(), dimidiumlabs_ui::CachePolicy::Revalidate);
        }
    }

    #[test]
    fn generated_license_bundle_includes_embedded_plex_fonts() {
        let license = LICENSES
            .licenses
            .iter()
            .find(|license| license.id == "OFL-1.1")
            .expect("OFL license");
        assert!(
            license
                .used_by
                .iter()
                .any(|usage| usage.crate_.name == "dimidiumlabs-ui")
        );
        assert!(license.text.contains("SIL OPEN FONT LICENSE Version 1.1"));
    }

    #[test]
    fn manifest_uses_registered_icon_urls() {
        let manifest = std::str::from_utf8(
            ASSETS
                .lookup("manifest.webmanifest")
                .expect("generated manifest")
                .asset()
                .bytes(),
        )
        .expect("UTF-8 manifest");
        assert!(manifest.contains("/-/assets/icon-192.png"));
        assert!(manifest.contains("/-/assets/icon-512.png"));
        assert!(!manifest.contains("\"/icon-"));
    }

    #[tokio::test]
    async fn dynamic_html_is_stream_compressed_with_weak_validators() {
        let router = Arc::new(WebController::new(None, None)).router();
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::ACCEPT_ENCODING, "gzip")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_ENCODING], "gzip");
        assert_eq!(response.headers()[header::VARY], "Accept-Encoding");
        let etag = response.headers()[header::ETAG].clone();
        assert!(etag.as_bytes().starts_with(b"W/\""));
        assert!(!response.headers().contains_key(header::CONTENT_LENGTH));

        let head = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::HEAD)
                    .uri("/")
                    .header(header::ACCEPT_ENCODING, "br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()[header::CONTENT_ENCODING], "br");
        assert!(head.headers()[header::ETAG].as_bytes().starts_with(b"W/\""));
        assert!(!head.headers().contains_key(header::CONTENT_LENGTH));
        assert!(
            to_bytes(head.into_body(), usize::MAX)
                .await
                .unwrap()
                .is_empty()
        );

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::ACCEPT_ENCODING, "br")
                    .header(header::IF_NONE_MATCH, &etag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(response.headers()[header::ETAG], etag);
        assert_eq!(response.headers()[header::VARY], "Accept-Encoding");
        assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
    }

    #[tokio::test]
    async fn shared_assets_keep_etag_cache_and_head_behavior() {
        let router = Arc::new(WebController::new(None, None)).router();
        let stylesheet = ASSETS.lookup("foundation.css").unwrap().asset();
        let stylesheet_path = format!("{ASSET_PREFIX}/{}", stylesheet.fingerprinted_name());
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&stylesheet_path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .is_none()
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        let etag = response.headers()[header::ETAG].clone();

        let compressed = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&stylesheet_path)
                    .header(header::ACCEPT_ENCODING, "br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(compressed.status(), StatusCode::OK);
        assert_eq!(compressed.headers()[header::CONTENT_ENCODING], "br");
        assert_eq!(compressed.headers()[header::VARY], "Accept-Encoding");
        assert_ne!(compressed.headers()[header::ETAG], etag);
        assert_eq!(
            compressed.headers()[header::CONTENT_LENGTH],
            stylesheet.brotli().unwrap().bytes().len().to_string()
        );

        let head = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::HEAD)
                    .uri(&stylesheet_path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()[header::ETAG], etag);
        assert!(
            to_bytes(head.into_body(), usize::MAX)
                .await
                .unwrap()
                .is_empty()
        );

        let cached = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&stylesheet_path)
                    .header(header::IF_NONE_MATCH, &etag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(cached.headers()[header::ETAG], etag);

        let font = ASSETS
            .assets()
            .iter()
            .find(|asset| {
                asset.name().contains("ibm-plex-mono") && asset.name().ends_with("roman.woff2")
            })
            .expect("foundation font");
        let font_path = format!("{ASSET_PREFIX}/{}", font.fingerprinted_name());
        let font = router
            .oneshot(
                Request::builder()
                    .uri(font_path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(font.status(), StatusCode::OK);
        assert_eq!(font.headers()[header::CONTENT_TYPE], "font/woff2");
        assert_eq!(
            font.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
    }
}
