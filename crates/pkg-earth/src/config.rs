// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use garde::Validate;
use ipnet::IpNet;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tracing::info;

use base::serde::deserialize_listener_addr;
use dimidiumlabs_config::{NonEmptyPath, NonEmptyString, NonZeroByteSize, NonZeroDuration};
use repos::{GoConfig, ZigConfig};

#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
pub struct ServerConfig {
    /// When receiving a SIGINT/SIGTERM signal, we will wait for the proposed timeout before terminating workers
    #[garde(dive)]
    pub shutdown_timeout: NonZeroDuration,

    /// Request timeout - maximum time to process a request (protects against Slowloris)
    #[garde(dive)]
    pub request_timeout: NonZeroDuration,

    /// Maximum request body size
    #[garde(dive)]
    pub max_body_size: NonZeroByteSize<usize>,

    /// Maximum number of concurrent requests across all clients
    #[garde(custom(validate_nonzero_usize))]
    pub max_concurrent_requests: usize,

    /// Rate limit: requests per second per client IP
    #[garde(dive)]
    pub rate_limit_period: NonZeroDuration,

    /// Rate limit: burst size (max requests allowed in a burst) per client IP
    #[garde(custom(validate_nonzero_u32))]
    pub rate_limit_burst_size: u32,

    /// Reverse-proxy networks allowed to supply `X-Forwarded-For`.
    #[serde(default)]
    #[garde(skip)]
    pub trusted_proxies: Vec<IpNet>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout: NonZeroDuration::from_secs(60),
            request_timeout: NonZeroDuration::from_secs(30),
            max_body_size: NonZeroByteSize::mb(64),
            max_concurrent_requests: 512,
            rate_limit_period: NonZeroDuration::from_secs(10),
            rate_limit_burst_size: 50,
            trusted_proxies: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
pub struct ListenerConfig {
    #[serde(deserialize_with = "deserialize_listener_addr")]
    #[garde(custom(|addr, _| validate_listener(addr, self)))]
    pub addr: SocketAddr,

    /// Hostnames to accept for this listener. Empty means accept all.
    #[garde(inner(custom(validate_hostname)))]
    pub hostnames: Vec<String>,

    /// Path to TLS certificate file (PEM format). If set, tls_key must also be set.
    #[garde(skip)]
    pub tls_crt: Option<PathBuf>,

    /// Path to TLS private key file (PEM format). If set, tls_crt must also be set.
    #[garde(skip)]
    pub tls_key: Option<PathBuf>,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:2025".parse().unwrap(),
            hostnames: vec![
                String::from("[::1]"),
                String::from("127.0.0.1"),
                String::from("localhost"),
            ],
            tls_crt: None,
            tls_key: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StdoutFormat {
    #[default]
    Pretty,
    Json,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StdoutConfig {
    /// Enables sending logs to the stdout
    pub enabled: bool,

    /// Controls which logs will be sent to stdout
    pub log_level: LogLevel,

    /// Controls the format of logs in stdout
    pub log_format: StdoutFormat,
}

impl Default for StdoutConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_level: LogLevel::Info,
            log_format: StdoutFormat::Pretty,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(default)]
#[garde(allow_unvalidated)]
pub struct OtelcolConfig {
    /// Enables sending telemetry to the otlp collector
    pub enabled: bool,

    /// Send logs to OTLP at this level (None = disabled)
    pub logs: bool,

    /// Send traces to OTLP
    pub traces: bool,

    /// Send traces to OTLP
    pub metrics: bool,

    /// OTLP endpoint (grpc:// or http://)
    pub endpoint: String,

    /// Export timeout as a human-readable duration.
    #[garde(dive)]
    pub timeout: NonZeroDuration,

    /// Controls which logs will be sent to otlp
    pub log_level: LogLevel,

    /// Path to CA certificate for TLS (required for grpcs://)
    pub tls_ca: Option<PathBuf>,

    /// Path to client certificate for mTLS
    pub tls_crt: Option<PathBuf>,

    /// Path to client key for mTLS
    pub tls_key: Option<PathBuf>,

    /// HTTP headers for authentication
    pub headers: HashMap<String, String>,
}

impl Default for OtelcolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            logs: true,
            traces: true,
            metrics: true,
            timeout: NonZeroDuration::from_secs(10),
            endpoint: "http://localhost:4317".into(),
            log_level: LogLevel::Info,
            tls_ca: None,
            tls_crt: None,
            tls_key: None,
            headers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default, Validate)]
#[serde(default)]
#[garde(allow_unvalidated)]
pub struct TelemetryConfig {
    pub stdout: StdoutConfig,
    #[garde(dive)]
    pub otelcol: Option<OtelcolConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct BackendsConfig {
    pub go: GoConfig,
    pub zig: ZigConfig,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(default)]
#[garde(allow_unvalidated)]
pub struct ConfigService {
    #[garde(dive, custom(validate_appname))]
    appname: NonEmptyString,
    #[garde(dive, custom(validate_dirname))]
    dirname: NonEmptyPath,
    #[garde(dive)]
    listen: Vec<ListenerConfig>,
    #[garde(dive)]
    server: ServerConfig,
    #[garde(dive)]
    telemetry: TelemetryConfig,
    // Backends have no service-specific validation policy.
    #[garde(skip)]
    backends: BackendsConfig,
}

impl Default for ConfigService {
    fn default() -> Self {
        Self {
            appname: NonEmptyString::from("pkg.earth"),
            dirname: NonEmptyPath::from("./.pkg-earth-state"),
            listen: vec![ListenerConfig::default()],
            server: ServerConfig::default(),
            telemetry: TelemetryConfig::default(),
            backends: BackendsConfig::default(),
        }
    }
}

impl ConfigService {
    pub async fn load(
        config_path: impl AsRef<Path>,
    ) -> Result<dimidiumlabs_config::Loaded<Self>, dimidiumlabs_config::Error> {
        let config_path = config_path.as_ref();
        info!(path = %config_path.display(), "use config file");
        dimidiumlabs_config::load(env!("CARGO_PKG_NAME"), config_path).await
    }

    pub fn appname(&self) -> &str {
        self.appname.as_str()
    }
    pub fn dirname(&self) -> &Path {
        self.dirname.as_path()
    }
    pub fn server(&self) -> &ServerConfig {
        &self.server
    }
    pub fn listeners(&self) -> &[ListenerConfig] {
        &self.listen
    }
    pub fn telemetry(&self) -> &TelemetryConfig {
        &self.telemetry
    }
    pub fn backends(&self) -> &BackendsConfig {
        &self.backends
    }
}

fn invalid(message: impl Into<String>) -> garde::Result {
    Err(garde::Error::new(message.into()))
}

fn validate_appname(appname: &NonEmptyString, _: &()) -> garde::Result {
    if appname
        .as_str()
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        invalid(format!(
            "appname '{appname}' contains invalid characters (only a-z, A-Z, 0-9, -, _ allowed)"
        ))
    }
}

fn validate_dirname(dirname: &NonEmptyPath, _: &()) -> garde::Result {
    let dirname = dirname.as_path();
    let metadata = fs::metadata(dirname).map_err(|error| {
        garde::Error::new(if error.kind() == std::io::ErrorKind::NotFound {
            format!("dirname '{}' does not exist", dirname.display())
        } else {
            format!("failed to read dirname '{}': {error}", dirname.display())
        })
    })?;
    if !metadata.is_dir() {
        return invalid(format!(
            "dirname '{}' is not a directory",
            dirname.display()
        ));
    }
    let testfile = dirname.join(".health");
    fs::write(&testfile, std::process::id().to_string()).map_err(|error| {
        garde::Error::new(format!(
            "dirname '{}' is not writable: {error}",
            dirname.display()
        ))
    })?;
    fs::remove_file(testfile).map_err(|error| garde::Error::new(error.to_string()))
}

fn validate_nonzero_usize(value: &usize, _: &()) -> garde::Result {
    if *value == 0 {
        invalid("server policy must not be zero")
    } else {
        Ok(())
    }
}

fn validate_nonzero_u32(value: &u32, _: &()) -> garde::Result {
    if *value == 0 {
        invalid("server policy must not be zero")
    } else {
        Ok(())
    }
}

fn validate_hostname(hostname: &str, _: &()) -> garde::Result {
    if hostname.parse::<hyper::http::uri::Authority>().is_ok() {
        Ok(())
    } else {
        invalid(format!("invalid hostname authority '{hostname}'"))
    }
}

fn validate_listener(_: &SocketAddr, listener: &ListenerConfig) -> garde::Result {
    match (&listener.tls_crt, &listener.tls_key) {
        (Some(_), None) => invalid(format!(
            "listener '{}': tls_crt is set but tls_key is missing",
            listener.addr
        )),
        (None, Some(_)) => invalid(format!(
            "listener '{}': tls_key is set but tls_crt is missing",
            listener.addr
        )),
        (Some(crt), Some(key)) if !crt.exists() => invalid(format!(
            "listener '{}': TLS crtificate file not found: {}",
            listener.addr,
            crt.display()
        )),
        (Some(_), Some(key)) if !key.exists() => invalid(format!(
            "listener '{}': TLS key file not found: {}",
            listener.addr,
            key.display()
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
impl ConfigService {
    pub fn for_test(dirname: PathBuf) -> Self {
        Self {
            appname: NonEmptyString::from("test"),
            dirname: NonEmptyPath::from(dirname),
            server: ServerConfig::default(),
            listen: vec![ListenerConfig {
                addr: "127.0.0.1:0".parse().unwrap(),
                hostnames: Vec::new(),
                tls_crt: None,
                tls_key: None,
            }],
            telemetry: TelemetryConfig::default(),
            backends: BackendsConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn base_config(dir: PathBuf) -> ConfigService {
        ConfigService::for_test(dir)
    }
    fn invalid_config(config: &ConfigService) -> String {
        config.validate().unwrap_err().to_string()
    }

    #[tokio::test]
    async fn defaults_and_load_file() {
        let dir = TempDir::new().unwrap();
        let state = dir.path().join("state");
        fs::create_dir(&state).unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, format!("appname = 'ok'\ndirname = '{}'\nlisten = []\n[server]\nshutdown_timeout = '1s'\nrequest_timeout = '1s'\nmax_body_size = '1 MB'\nmax_concurrent_requests = 1\nrate_limit_period = '1s'\nrate_limit_burst_size = 1\ntrusted_proxies = ['10.2.0.0/16']\n[backends.go]\nrefresh_interval = '1h'\n[telemetry.otelcol]\ntimeout = '10s'\n", state.display())).unwrap();
        let loaded = ConfigService::load(&path).await.unwrap();
        assert_eq!(loaded.metadata().path(), path);
        assert_eq!(loaded.config().appname(), "ok");
        assert_eq!(
            loaded.config().server().trusted_proxies,
            vec!["10.2.0.0/16".parse().unwrap()]
        );
        assert_eq!(
            loaded
                .config()
                .backends()
                .go
                .refresh_interval
                .get()
                .as_secs(),
            60 * 60
        );
        assert_eq!(
            loaded
                .config()
                .telemetry()
                .otelcol
                .as_ref()
                .unwrap()
                .timeout
                .get()
                .as_secs(),
            10
        );
    }

    #[tokio::test]
    async fn load_reports_shared_errors() {
        let dir = TempDir::new().unwrap();
        let error = ConfigService::load(dir.path().join("missing.toml"))
            .await
            .unwrap_err();
        assert!(matches!(error, dimidiumlabs_config::Error::Read { .. }));

        let malformed = dir.path().join("malformed.toml");
        fs::write(&malformed, "appname = [").unwrap();
        let error = ConfigService::load(&malformed).await.unwrap_err();
        assert!(matches!(
            error,
            dimidiumlabs_config::Error::Toml { metadata, .. }
                if metadata.path() == malformed
        ));
    }

    #[tokio::test]
    async fn load_preserves_validation_metadata() {
        let dir = TempDir::new().unwrap();
        let state = dir.path().join("state");
        fs::create_dir(&state).unwrap();
        let path = dir.path().join("invalid.toml");
        fs::write(
            &path,
            format!(
                "appname = 'not valid'\ndirname = '{}'\nlisten = []\n[server]\nshutdown_timeout = '1s'\nrequest_timeout = '1s'\nmax_body_size = '1 MB'\nmax_concurrent_requests = 1\nrate_limit_period = '1s'\nrate_limit_burst_size = 1\n",
                state.display()
            ),
        )
        .unwrap();

        let error = ConfigService::load(&path).await.unwrap_err();
        match error {
            dimidiumlabs_config::Error::Validation { source, metadata } => {
                assert_eq!(metadata.path(), path);
                assert_eq!(metadata.format(), dimidiumlabs_config::Format::Toml);
                assert!(source.to_string().contains("appname"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn loads_json_and_yaml() {
        let dir = TempDir::new().unwrap();
        let state = dir.path().join("state");
        fs::create_dir(&state).unwrap();
        let json = dir.path().join("config.json");
        fs::write(
            &json,
            format!(
                r#"{{"appname":"json","dirname":"{}","listen":[],"server":{{"shutdown_timeout":"1s","request_timeout":"1s","max_body_size":"1 MB","max_concurrent_requests":1,"rate_limit_period":"1s","rate_limit_burst_size":1}}}}"#,
                state.display()
            ),
        )
        .unwrap();
        let yaml = dir.path().join("config.yaml");
        fs::write(
            &yaml,
            format!(
                "appname: yaml\ndirname: '{}'\nlisten: []\nserver:\n  shutdown_timeout: 1s\n  request_timeout: 1s\n  max_body_size: 1 MB\n  max_concurrent_requests: 1\n  rate_limit_period: 1s\n  rate_limit_burst_size: 1\n",
                state.display()
            ),
        )
        .unwrap();

        assert_eq!(
            ConfigService::load(json).await.unwrap().config().appname(),
            "json"
        );
        assert_eq!(
            ConfigService::load(yaml).await.unwrap().config().appname(),
            "yaml"
        );
    }

    #[tokio::test]
    async fn rejects_numeric_durations_and_unitless_byte_sizes() {
        let dir = TempDir::new().unwrap();
        let state = dir.path().join("state");
        fs::create_dir(&state).unwrap();
        for (extension, source) in [
            (
                "toml",
                format!(
                    "appname = 'test'\ndirname = '{}'\nlisten = []\n[server]\nshutdown_timeout = 1\n",
                    state.display()
                ),
            ),
            (
                "json",
                format!(
                    r#"{{"appname":"test","dirname":"{}","listen":[],"server":{{"shutdown_timeout":"1s","request_timeout":"1s","max_body_size":1024,"max_concurrent_requests":1,"rate_limit_period":"1s","rate_limit_burst_size":1}}}}"#,
                    state.display()
                ),
            ),
            (
                "yaml",
                format!(
                    "appname: test\ndirname: '{}'\nlisten: []\nserver:\n  shutdown_timeout: 1s\n  request_timeout: 1s\n  max_body_size: '1024'\n  max_concurrent_requests: 1\n  rate_limit_period: 1s\n  rate_limit_burst_size: 1\n",
                    state.display()
                ),
            ),
        ] {
            let path = dir.path().join(format!("invalid.{extension}"));
            fs::write(&path, source).unwrap();
            assert!(ConfigService::load(path).await.is_err());
        }
    }

    #[test]
    fn validates_appname_directory_and_server_policies() {
        let dir = TempDir::new().unwrap();
        let mut config = base_config(dir.path().to_path_buf());
        config.appname = "bad name!".into();
        assert!(invalid_config(&config).contains("appname 'bad name!'"));
        let mut config = base_config(dir.path().to_path_buf());
        config.appname = "".into();
        let report = invalid_config(&config);
        assert!(report.contains("appname"));
        assert!(report.contains("must not be empty"));
        let mut config = base_config(dir.path().to_path_buf());
        config.dirname = "".into();
        let report = invalid_config(&config);
        assert!(report.contains("dirname"));
        assert!(report.contains("must not be empty"));
        let config = base_config(dir.path().join("missing"));
        assert!(invalid_config(&config).contains("does not exist"));
        let file = dir.path().join("file");
        fs::write(&file, "x").unwrap();
        assert!(invalid_config(&base_config(file)).contains("is not a directory"));
        let mut config = base_config(dir.path().to_path_buf());
        config.server.shutdown_timeout = NonZeroDuration::ZERO;
        assert!(invalid_config(&config).contains("must be at least 1 nanoseconds"));
        let mut config = base_config(dir.path().to_path_buf());
        config.server.request_timeout = NonZeroDuration::ZERO;
        assert!(invalid_config(&config).contains("must be at least 1 nanoseconds"));
        let mut config = base_config(dir.path().to_path_buf());
        config.server.max_concurrent_requests = 0;
        assert!(invalid_config(&config).contains("server policy must not be zero"));
        let mut config = base_config(dir.path().to_path_buf());
        config.server.rate_limit_period = NonZeroDuration::ZERO;
        assert!(invalid_config(&config).contains("must be at least 1 nanoseconds"));
        let mut config = base_config(dir.path().to_path_buf());
        config.server.rate_limit_burst_size = 0;
        assert!(invalid_config(&config).contains("server policy must not be zero"));
    }

    #[test]
    fn validates_hostnames_and_tls_pairs_and_files() {
        let dir = TempDir::new().unwrap();
        let mut config = base_config(dir.path().to_path_buf());
        config.listen[0].hostnames = vec!["bad host".into()];
        assert!(invalid_config(&config).contains("invalid hostname authority"));
        let mut config = base_config(dir.path().to_path_buf());
        config.listen[0].tls_crt = Some(dir.path().join("cert"));
        assert!(invalid_config(&config).contains("tls_key is missing"));
        let mut config = base_config(dir.path().to_path_buf());
        config.listen[0].tls_key = Some(dir.path().join("key"));
        assert!(invalid_config(&config).contains("tls_crt is missing"));
        let mut config = base_config(dir.path().to_path_buf());
        config.listen[0].tls_crt = Some(dir.path().join("cert"));
        config.listen[0].tls_key = Some(dir.path().join("key"));
        assert!(invalid_config(&config).contains("TLS crtificate file not found"));
        fs::write(dir.path().join("cert"), "x").unwrap();
        assert!(invalid_config(&config).contains("TLS key file not found"));
        fs::write(dir.path().join("key"), "x").unwrap();
        config.validate().unwrap();
    }
}
