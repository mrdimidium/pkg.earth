// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

mod config;
mod logging;
mod proxy;
mod repos;
mod storage;
mod ui;

mod controller_backend;
mod controller_web;

use std::net::SocketAddr;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::{Body, HttpBody as _},
    extract::ConnectInfo,
    http::{self, Request},
    middleware::Next,
    response::Response,
};
use axum_server::tls_rustls::RustlsConfig;
use dimidiumlabs_server::{
    service::{
        AdmissionLayer, ClientIp, ClientIpKeyExtractor, ClientIpLayer, DrainLayer, ForwardedHeader,
        HostLayer, HostPattern, HstsLayer, PeerAddr, RateLimitLayer, TrustedProxies, rate_limit,
    },
    transport::HttpTransport,
};
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use log::{error, info, trace};
#[cfg(target_os = "linux")]
use sd_notify::NotifyState;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    signal,
};
use tokio_rustls::TlsAcceptor;

use crate::controller_backend::BackendController;
use crate::controller_web::WebController;
use crate::repos::{Backend, BackendSpec, GoBackend, ZigBackend};

async fn init_backend<S: BackendSpec>(
    backend: Backend<S>,
    index_tasks: &mut tokio::task::JoinSet<()>,
    index_cancel: tokio_util::sync::CancellationToken,
) -> Option<Arc<Backend<S>>> {
    if !backend.enabled() {
        return None;
    }
    let backend = Arc::new(backend);
    let interval = backend.refresh_interval();
    if !interval.is_zero() {
        index_tasks.spawn(run_index_refresh(
            S::ID,
            backend.clone(),
            interval,
            index_cancel,
        ));
    }
    Some(backend)
}

async fn run_index_refresh<S: BackendSpec>(
    name: &'static str,
    backend: Arc<Backend<S>>,
    interval: std::time::Duration,
    cancel: tokio_util::sync::CancellationToken,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("index refresh stopped; backend={name}");
                break;
            }
            _ = ticker.tick() => {
                info!("refreshing index; backend={name}");
                match backend.refresh().await {
                    Ok(()) => info!("index refreshed; backend={name}"),
                    Err(error) => error!("index refresh failed; backend={name} error={error}"),
                }
            }
        }
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
const REQUEST_ID_HEADER: http::HeaderName = http::HeaderName::from_static("x-request-id");
const HTTP_HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP1_MAX_BUFFER_BYTES: usize = 32 * 1024;
const HTTP2_MAX_CONCURRENT_STREAMS: u32 = 128;
const HTTP2_MAX_HEADER_LIST_BYTES: u32 = 32 * 1024;
const HTTP2_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(30);
const HTTP2_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(10);
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const HSTS_POLICY: &str = "max-age=63072000; includeSubDomains; preload";
const HELP: &str = "\
Usage: pkg-earth --config=<path>

Options:
  --config=<path>    Path to config file (required)
  --help             Show this help message
  --version          Show version
";

fn parse_config_path(args: &[String]) -> Result<PathBuf, &'static str> {
    let mut config_path = None;
    for arg in args {
        if let Some(path) = arg.strip_prefix("--config=") {
            if path.is_empty() {
                return Err("--config=<path> requires a non-empty path");
            }
            if config_path.replace(PathBuf::from(path)).is_some() {
                return Err("only one --config=<path> may be supplied");
            }
        }
    }
    config_path.ok_or("--config=<path> is required")
}

/// Contains metainfo about one server interface
#[derive(Clone)]
struct ListenerInfo {
    addr: SocketAddr,
}

async fn log_request(request: Request<Body>, next: Next) -> Response {
    let started_at = std::time::Instant::now();
    let request_id = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<invalid>")
        .to_owned();
    let local_addr = request
        .extensions()
        .get::<ListenerInfo>()
        .map(|info| info.addr);
    let remote_addr = request
        .extensions()
        .get::<ClientIp>()
        .map(|client| client.0);
    let method = request.method().clone();
    let version = request.version();
    let path = request.uri().clone();
    let host = extract_host(&request);
    let user_agent = request
        .headers()
        .get(http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(String::from);

    let response = next.run(request).await;
    let status = response.status().as_u16();
    let content_length = response.body().size_hint().exact();
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());

    info!(
        "HTTP request; request_id={request_id} local_addr={local_addr:?} remote_addr={remote_addr:?} method={method} version={version:?} path={path} host={host:?} user_agent={user_agent:?} status={status} latency_ns={} content_type={content_type:?} content_length={content_length:?}",
        started_at.elapsed().as_nanos(),
    );

    response
}

#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{HELP}");
        return;
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("pkg-earth {VERSION}");
        return;
    }

    let config_path = parse_config_path(&args).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(1);
    });
    let loaded_config = config::ConfigService::load(config_path)
        .await
        .unwrap_or_else(|error| {
            eprintln!("invalid config: {error}");
            std::process::exit(1);
        });
    let (config, metadata) = loaded_config.into_parts();
    logging::init(config.log());
    info!("using config file; path={}", metadata.path().display());
    let config = Arc::new(config);

    let storage = Arc::new(storage::StorageService::new(config.clone()).await.unwrap());
    let network = Arc::new(proxy::ProxyService::new());

    let source = format!("pkg.earth:{}", config.appname());
    let backends = config.backends();

    let rate_limit_config = Arc::new(
        rate_limit(
            config.server().rate_limit_period.get(),
            NonZeroU32::new(config.server().rate_limit_burst_size)
                .expect("rate-limit burst is validated"),
            ClientIpKeyExtractor,
        )
        .expect("rate-limit policy is validated"),
    );
    let admission = AdmissionLayer::new(
        NonZeroUsize::new(config.server().max_concurrent_requests)
            .expect("concurrency limit is validated"),
    );
    let client_ip = ClientIpLayer::new(TrustedProxies::new(
        config.server().trusted_proxies.iter().copied(),
        ForwardedHeader::XForwardedFor,
    ));
    let (drain_layer, drain_handle) = DrainLayer::new();
    let transport = HttpTransport::new(
        HTTP_HEADER_READ_TIMEOUT,
        HTTP1_MAX_BUFFER_BYTES,
        NonZeroU32::new(HTTP2_MAX_CONCURRENT_STREAMS).expect("HTTP/2 stream limit is non-zero"),
        NonZeroU32::new(HTTP2_MAX_HEADER_LIST_BYTES).expect("HTTP/2 header limit is non-zero"),
    )
    .expect("HTTP transport policy is valid")
    .with_http2_keep_alive(HTTP2_KEEP_ALIVE_INTERVAL, HTTP2_KEEP_ALIVE_TIMEOUT)
    .expect("HTTP/2 keep-alive policy is valid");

    let mut index_tasks = tokio::task::JoinSet::new();
    let index_cancel = tokio_util::sync::CancellationToken::new();

    let zig_backend = init_backend(
        ZigBackend::new(
            backends.zig.clone(),
            source.clone(),
            storage.clone(),
            network.clone(),
        ),
        &mut index_tasks,
        index_cancel.clone(),
    )
    .await;

    let go_backend = init_backend(
        GoBackend::new(
            backends.go.clone(),
            source.clone(),
            storage.clone(),
            network.clone(),
        ),
        &mut index_tasks,
        index_cancel.clone(),
    )
    .await;

    let web_controller = Arc::new(WebController::new(zig_backend.clone(), go_backend.clone()));
    let mut app = axum::Router::new().merge(web_controller.router());

    if let Some(ref backend) = zig_backend {
        let ctrl = Arc::new(BackendController::new(
            backend.clone(),
            storage.clone(),
            network.clone(),
        ));
        app = app.nest("/zig", ctrl.router());
    }

    if let Some(ref backend) = go_backend {
        let ctrl = Arc::new(BackendController::new(
            backend.clone(),
            storage.clone(),
            network.clone(),
        ));
        app = app.nest("/go", ctrl.router());
    }

    let mut tasks = tokio::task::JoinSet::new();
    let server_cancel = tokio_util::sync::CancellationToken::new();

    for listener_config in config.listeners() {
        let listener = tokio::net::TcpListener::bind(listener_config.addr)
            .await
            .expect("failed to bind HTTP listener");
        let addr = listener.local_addr().expect("listener has a local address");
        let hosts = listener_config
            .hostnames
            .iter()
            .map(|host| HostPattern::new(host))
            .collect::<Result<Vec<_>, _>>()
            .expect("listener hostnames are validated");

        let listener_app = app.clone();
        let listener_app = if hosts.is_empty() {
            listener_app
        } else {
            listener_app.layer(HostLayer::new(hosts))
        };
        let listener_app = listener_app
            .layer(tower_http::limit::RequestBodyLimitLayer::new(
                config.server().max_body_size.get(),
            ))
            .layer(tower_http::timeout::TimeoutLayer::with_status_code(
                http::StatusCode::REQUEST_TIMEOUT,
                config.server().request_timeout.get(),
            ))
            .layer(axum::middleware::from_fn(log_request))
            .layer(RateLimitLayer::new(rate_limit_config.clone()))
            .layer(admission.clone())
            .layer(client_ip.clone())
            .layer(drain_layer.clone())
            .layer(tower_http::request_id::PropagateRequestIdLayer::new(
                REQUEST_ID_HEADER.clone(),
            ))
            .layer(tower_http::request_id::SetRequestIdLayer::new(
                REQUEST_ID_HEADER.clone(),
                tower_http::request_id::MakeRequestUuid,
            ))
            .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
                http::header::SERVER,
                http::HeaderValue::from_static(concat!("pkg.earth/", env!("CARGO_PKG_VERSION"))),
            ))
            .layer(axum::Extension(ListenerInfo { addr }));

        let tls =
            if let (Some(crt), Some(key)) = (&listener_config.tls_crt, &listener_config.tls_key) {
                let rustls_config = RustlsConfig::from_pem_file(crt, key)
                    .await
                    .expect("failed to load TLS config");
                Some(TlsAcceptor::from(rustls_config.get_inner()))
            } else {
                None
            };
        let tls_enabled = tls.is_some();
        let listener_app = if tls_enabled {
            listener_app.layer(HstsLayer::new(http::HeaderValue::from_static(HSTS_POLICY)))
        } else {
            listener_app
        };
        let transport = transport.clone();
        let cancel = server_cancel.clone();
        tasks.spawn(async move {
            if let Err(error) = serve_listener(listener, listener_app, transport, tls, cancel).await
            {
                error!("listener failed; addr={addr} error={error}");
            }
        });

        info!(
            "listening {} on {} (hostnames: {})",
            if tls_enabled { "HTTPS" } else { "HTTP" },
            addr,
            if listener_config.hostnames.is_empty() {
                "*".to_string()
            } else {
                listener_config.hostnames.join(", ")
            },
        );
    }

    let mut watchdog_ticker = tokio::time::interval(std::time::Duration::from_secs(60));

    #[cfg(target_os = "linux")]
    if sd_notify::booted().unwrap_or(false) {
        sd_notify::notify(false, &[NotifyState::Ready]).ok();

        let mut usec = 0u64;
        (sd_notify::watchdog_enabled(true, &mut usec) && usec > 0).then(|| {
            let interval = std::time::Duration::from_micros(usec) / 2;
            info!("watchdog enabled; interval_ms={}", interval.as_millis());
            watchdog_ticker = tokio::time::interval(interval);
        });
    };

    #[cfg(unix)]
    let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
        .expect("failed to install signal handler");
    #[cfg(windows)]
    let mut sigint = signal::windows::signal(signal::windows::SignalKind::interrupt())
        .expect("failed to install signal handler");

    #[cfg(unix)]
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .expect("failed to install signal handler");

    loop {
        let watchdog = watchdog_ticker.tick();

        #[cfg(unix)]
        let sigterm = sigterm.recv();
        #[cfg(not(unix))]
        let sigterm = std::future::pending::<()>();

        tokio::select! {
            _ = sigint.recv() => {
                info!("received SIGINT, shutting down");
                break;
            },
            _ = sigterm => {
                info!("received SIGTERM, shutting down");
                break;
            },
            _ = watchdog => {
                trace!("server is alive");
                rate_limit_config.limiter().retain_recent();

                #[cfg(target_os = "linux")]
                sd_notify::notify(false, &[NotifyState::Watchdog]).ok();
            },
            result = tasks.join_next() => {
                match result {
                    Some(Ok(())) => error!("listener exited unexpectedly, shutting down"),
                    Some(Err(e)) => error!("listener failed: {e}, shutting down"),
                    None => {
                        error!("no listeners running");
                        return;
                    }
                }
                break;
            },
        }
    }

    #[cfg(target_os = "linux")]
    sd_notify::notify(false, &[NotifyState::Stopping]).ok();

    let _ = drain_handle.begin();
    server_cancel.cancel();
    index_cancel.cancel();

    // Wait for listeners, streaming response bodies, and index tasks to finish.
    let shutdown_result = tokio::time::timeout(config.server().shutdown_timeout.get(), async {
        while let Some(result) = tasks.join_next().await {
            if let Err(e) = result {
                error!("listener task failed: {e}");
            }
        }
        drain_handle.wait().await;
        while let Some(result) = index_tasks.join_next().await {
            if let Err(e) = result {
                error!("index task failed: {e}");
            }
        }
    })
    .await;

    if shutdown_result.is_err() {
        error!(
            "shutdown timeout after {:?}, aborting remaining tasks",
            config.server().shutdown_timeout,
        );
        tasks.abort_all();
        index_tasks.abort_all();
    } else {
        info!("shutdown complete");
    }
}

async fn serve_listener(
    listener: tokio::net::TcpListener,
    app: axum::Router,
    transport: HttpTransport,
    tls: Option<TlsAcceptor>,
    shutdown: tokio_util::sync::CancellationToken,
) -> std::io::Result<()> {
    let mut connections = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            () = shutdown.cancelled() => break,
            accepted = listener.accept() => {
                let (stream, peer) = accepted?;
                let app = app
                    .clone()
                    .layer(axum::Extension(ConnectInfo(peer)))
                    .layer(axum::Extension(PeerAddr(peer)));
                let transport = transport.clone();
                let tls = tls.clone();
                let shutdown = shutdown.clone();
                connections.spawn(async move {
                    if let Some(tls) = tls {
                        match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, tls.accept(stream)).await {
                            Ok(Ok(stream)) => serve_connection(stream, app, transport, shutdown).await,
                            Ok(Err(error)) => trace!("TLS handshake failed; peer={peer} error={error}"),
                            Err(_) => trace!("TLS handshake timed out; peer={peer}"),
                        }
                    } else {
                        serve_connection(stream, app, transport, shutdown).await;
                    }
                });
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                if let Err(error) = result {
                    error!("HTTP connection task failed; error={error}");
                }
            }
        }
    }

    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            error!("HTTP connection task failed; error={error}");
        }
    }
    Ok(())
}

async fn serve_connection<IO>(
    stream: IO,
    app: axum::Router,
    transport: HttpTransport,
    shutdown: tokio_util::sync::CancellationToken,
) where
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let builder = transport.builder();
    let connection =
        builder.serve_connection_with_upgrades(TokioIo::new(stream), TowerToHyperService::new(app));
    tokio::pin!(connection);
    let result = tokio::select! {
        result = &mut connection => result,
        () = shutdown.cancelled() => {
            connection.as_mut().graceful_shutdown();
            connection.await
        }
    };
    if let Err(error) = result {
        trace!("HTTP connection failed; error={error}");
    }
}

fn extract_host(req: &Request<Body>) -> Option<String> {
    // HTTP/1.1 uses HOST header, HTTP/2 uses :authority (available via URI)
    let raw = if let Some(host) = req.headers().get(http::header::HOST) {
        host.to_str().ok().map(|raw| {
            if let Some((host, port)) = raw.rsplit_once(':')
                && port.parse::<u16>().is_ok()
                && (host.ends_with(']') || !host.contains('['))
            {
                host
            } else {
                raw
            }
        })
    } else {
        req.uri().host()
    };

    raw.and_then(|raw| url::Host::parse(raw).ok())
        .map(|h| h.to_string().trim_end_matches('.').to_string())
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn parses_required_single_config_path() {
        assert_eq!(parse_config_path(&[]), Err("--config=<path> is required"));
        assert_eq!(
            parse_config_path(&["--config=".into()]),
            Err("--config=<path> requires a non-empty path")
        );
        assert_eq!(
            parse_config_path(&["--config=config.toml".into()]).unwrap(),
            PathBuf::from("config.toml")
        );
        assert_eq!(
            parse_config_path(&["--config=one.toml".into(), "--config=two.toml".into()]),
            Err("only one --config=<path> may be supplied")
        );
    }

    use super::*;

    #[tokio::test]
    async fn shared_transport_serves_with_connection_extensions() {
        let app = axum::Router::new().route(
            "/peer",
            axum::routing::get(
                |axum::Extension(peer): axum::Extension<PeerAddr>| async move {
                    peer.0.ip().to_string()
                },
            ),
        );
        let transport = HttpTransport::new(
            HTTP_HEADER_READ_TIMEOUT,
            HTTP1_MAX_BUFFER_BYTES,
            NonZeroU32::new(HTTP2_MAX_CONCURRENT_STREAMS).unwrap(),
            NonZeroU32::new(HTTP2_MAX_HEADER_LIST_BYTES).unwrap(),
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let shutdown = tokio_util::sync::CancellationToken::new();
        let server = tokio::spawn(serve_listener(
            listener,
            app,
            transport,
            None,
            shutdown.clone(),
        ));

        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"GET /peer HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let response = String::from_utf8(response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("127.0.0.1"));

        shutdown.cancel();
        server.await.unwrap().unwrap();
    }
}
