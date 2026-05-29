use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub postgresql: PostgreSqlConfig,
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: Option<LoggingConfig>,
    pub jwt: JwtConfig,
    #[serde(default)]
    pub app: Option<AppConfig>,
    #[serde(default)]
    pub code_sandbox: Option<CodeSandboxConfig>,
    #[serde(default)]
    pub secrets: Option<SecretsConfig>,
    /// Per-cache path overrides. Defaults to all-None; `Config::resolve_paths`
    /// fills each unset field with a subdir of `app.data_dir`. Operators
    /// override individual entries to put a particular cache on a
    /// different disk (e.g. `hf_models_dir` on a big spinning disk while
    /// `git_cache_dir` stays on the SSD).
    #[serde(default)]
    pub caches: CachesConfig,
}

/// Overridable paths for runtime caches. Every field defaults to a
/// subdir of `app.data_dir` after `Config::resolve_paths` runs. Direct
/// reads of these fields BEFORE `resolve_paths` see `None` — every
/// caller should be downstream of `Config::load_from` which calls it.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct CachesConfig {
    /// HuggingFace model downloads. Default `<app.data_dir>/hf-models`.
    /// Was hardcoded `~/.llm-runtime/models/` in the standalone crate.
    #[serde(default)]
    pub hf_models_dir: Option<String>,
    /// llama-server / mistralrs-server downloaded engine binaries.
    /// Default `<app.data_dir>/llm-engines`.
    /// Was hardcoded `~/.llm-runtime/binaries/` in the standalone crate.
    #[serde(default)]
    pub llm_engines_dir: Option<String>,
    /// Hub repository clones. Default `<app.data_dir>/cache/git`.
    /// Was hardcoded `dirs::cache_dir()/ziee/models/git/` in
    /// `utils/git/service.rs`.
    #[serde(default)]
    pub git_cache_dir: Option<String>,
    /// Git LFS object cache. Default `<app.data_dir>/cache/lfs`.
    /// Was nested under `git_cache_dir/lfs_cache` historically.
    #[serde(default)]
    pub lfs_cache_dir: Option<String>,
}

/// At-rest encryption configuration.
///
/// `storage_key` is a 32+ char passphrase used by pgcrypto's
/// pgp_sym_encrypt / pgp_sym_decrypt to wrap secret columns
/// (llm_providers.api_key_encrypted, user_llm_provider_api_keys.api_key_encrypted,
/// llm_repositories.auth_config_encrypted). When unset, the application
/// boots in compat mode — new writes stay in the plaintext columns and
/// a tracing::warn is emitted at startup. Closes 06-llm-provider F-02
/// (Critical) once configured.
#[derive(Debug, Deserialize, Clone)]
pub struct SecretsConfig {
    /// Symmetric passphrase passed to pgp_sym_encrypt. Must be 32+ chars.
    /// In production, set via env var; in dev / tests, the dev.yaml /
    /// test config carries a fixed value so the round-trip works.
    #[serde(default)]
    pub storage_key: Option<String>,
}

/// Configuration for the code_sandbox built-in MCP server.
///
/// Disabled by default so dev environments without bwrap / rootfs boot cleanly.
/// Flip `enabled` to true after bwrap is installed and the rootfs is mounted.
#[derive(Debug, Deserialize, Clone)]
pub struct CodeSandboxConfig {
    /// Master switch. When false, the module's `init()` returns early
    /// (no boot probes, no MCP row upsert, no reaper task).
    #[serde(default)]
    pub enabled: bool,
    /// Path to the cached rootfs squashfs files. Default
    /// `<app.data_dir>/sandbox-rootfs/` (filled by `resolve_paths`).
    /// Bind-mounted read-only into every bwrap call.
    #[serde(default)]
    pub rootfs_path: Option<String>,
    /// Per-conversation sandbox workspaces root. Default
    /// `<app.data_dir>/sandboxes/`. Was previously derived ad-hoc in
    /// `code_sandbox/mod.rs` with no override.
    #[serde(default)]
    pub workspace_root: Option<String>,
    /// Delegated cgroup v2 parent. Empty string → rlimits-only mode
    /// (no per-call cgroup scope; rlimits still enforce memory + procs).
    #[serde(default)]
    pub cgroup_parent: String,
    /// When true (default), the FIRST `execute_command` for a flavor
    /// whose rootfs isn't cached yet prompts the user for consent (via
    /// an MCP elicitation) before starting the multi-hundred-MB
    /// download. Set false to always auto-download silently.
    #[serde(default = "default_require_download_consent")]
    pub require_download_consent: bool,
    /// Flavors whose advertised size is below this threshold (MiB) skip
    /// the consent prompt and download silently — so the small
    /// `minimal` rootfs stays frictionless while a large `full` rootfs
    /// always asks. Only consulted when `require_download_consent` is true.
    #[serde(default = "default_auto_download_under_mb")]
    pub auto_download_under_mb: u64,
    /// Audit H-8: refuse to register the sandbox MCP server on Windows when
    /// WSL2 `networkingMode = mirrored` is enabled in `.wslconfig`. In
    /// mirrored mode the Windows host's 127.0.0.1 (Postgres, the Ziee API
    /// itself, browser DevTools, …) is reachable from inside the distro,
    /// and `--share-net` carries that into the sandbox. The previous
    /// behavior was a warn-log only. Operators who genuinely want this
    /// configuration must opt in with `allow_wsl2_mirrored_mode: true`.
    /// No-op on Linux/macOS.
    #[serde(default)]
    pub allow_wsl2_mirrored_mode: bool,
    /// Audit H-4: refuse to register when the cloud instance metadata
    /// service (169.254.169.254) is reachable from the host AND `--share-net`
    /// would expose it to LLM-generated code. On EC2/GCE/Azure, IMDS hands
    /// out IAM/role credentials that the sandboxed workload can curl + ship
    /// to whatever egress the LLM is told to use. Operators on a cloud host
    /// who genuinely want this configuration (e.g. behind IMDSv2 with a
    /// hop-limit of 1 + sandboxed bash unable to set the v2 token header)
    /// must opt in with `allow_cloud_imds_reachable: true`. No-op on hosts
    /// where IMDS is unreachable (the common dev / on-prem case).
    #[serde(default)]
    pub allow_cloud_imds_reachable: bool,
}

impl Default for CodeSandboxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rootfs_path: None,
            workspace_root: None,
            cgroup_parent: String::new(),
            require_download_consent: default_require_download_consent(),
            auto_download_under_mb: default_auto_download_under_mb(),
            allow_wsl2_mirrored_mode: false,
            allow_cloud_imds_reachable: false,
        }
    }
}

impl CodeSandboxConfig {
    /// `rootfs_path` after `Config::resolve_paths` has run. Panics if
    /// called on an unresolved config (programmer error — `load_from`
    /// always resolves).
    pub fn rootfs_path(&self) -> &str {
        self.rootfs_path
            .as_deref()
            .expect("rootfs_path filled by Config::resolve_paths")
    }
    pub fn workspace_root(&self) -> &str {
        self.workspace_root
            .as_deref()
            .expect("workspace_root filled by Config::resolve_paths")
    }
}

impl CachesConfig {
    pub fn hf_models_dir(&self) -> &str {
        self.hf_models_dir
            .as_deref()
            .expect("hf_models_dir filled by Config::resolve_paths")
    }
    pub fn llm_engines_dir(&self) -> &str {
        self.llm_engines_dir
            .as_deref()
            .expect("llm_engines_dir filled by Config::resolve_paths")
    }
    pub fn git_cache_dir(&self) -> &str {
        self.git_cache_dir
            .as_deref()
            .expect("git_cache_dir filled by Config::resolve_paths")
    }
    pub fn lfs_cache_dir(&self) -> &str {
        self.lfs_cache_dir
            .as_deref()
            .expect("lfs_cache_dir filled by Config::resolve_paths")
    }
}

fn default_require_download_consent() -> bool {
    true
}

fn default_auto_download_under_mb() -> u64 {
    100
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub data_dir: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostgreSqlConfig {
    pub use_embedded: bool,
    #[serde(default)]
    pub embedded: Option<EmbeddedPostgreSqlConfig>,
    #[serde(default)]
    pub external: Option<ExternalPostgreSqlConfig>,
    #[serde(default)]
    pub pool: Option<PoolConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EmbeddedPostgreSqlConfig {
    pub version: String,
    pub port: u16,
    pub bind_address: String,
    pub username: String,
    pub password: String,
    pub database: String,
    /// Postgres install tree (bin/lib/share). Default
    /// `<app.data_dir>/postgres/` (filled by `resolve_paths`).
    /// postgresql_embedded skips re-extraction if the version matches —
    /// safe to share across server upgrades.
    #[serde(default)]
    pub installation_dir: Option<String>,
    /// PGDATA cluster (pg_wal, base, postgresql.conf). Default
    /// `<app.data_dir>/postgres-data/`. Operators commonly override
    /// to put the cluster on a fast disk.
    #[serde(default)]
    pub data_dir: Option<String>,
    pub timezone: String,
    pub log_timezone: String,
    pub logging: LoggingConfigPostgres,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExternalPostgreSqlConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfigPostgres {
    pub collector: bool,
    pub directory: String,
    pub filename: String,
    pub statement: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_secs: u64,
    #[serde(default)]
    pub idle_timeout_secs: Option<u64>,
    #[serde(default)]
    pub max_lifetime_secs: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub api_prefix: String,
    #[serde(default)]
    pub cors: Option<CorsConfig>,
    /// Rate-limit configuration (tower-governor). Optional — defaults
    /// match the A3 hardening posture (5 req/s sustained, 60-burst).
    /// Tests override with much higher numbers since they run many
    /// sequential requests against 127.0.0.1 (single peer-IP bucket).
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
    /// Honor X-Forwarded-Host / X-Forwarded-Proto in OAuth
    /// redirect_uri derivation.
    ///
    /// **Default false.** Only set true when the server is behind a
    /// reverse proxy that STRIPS inbound X-Forwarded-* headers and
    /// sets them itself (nginx `proxy_set_header`, Caddy `header_up`,
    /// Cloudflare / Vercel / Fly defaults, the Vite dev proxy in
    /// this repo's vite.config.ts). When the server is exposed
    /// directly, this MUST stay false — otherwise an attacker can
    /// send `X-Forwarded-Host: evil.com` to the backend and a
    /// permissive IdP (Keycloak wildcard, Dex, Authentik) will hand
    /// the OAuth `code` to evil.com. F-07 attack class.
    #[serde(default)]
    pub trust_forwarded_headers: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitConfig {
    /// Master on/off switch for the global tower-governor rate limiter.
    /// Defaults to `true` (preserve the A3 DoS-protection posture).
    ///
    /// Set `false` on trusted / non-public deployments. The built-in
    /// code_sandbox + memory MCP servers are reached over loopback
    /// (`http://127.0.0.1`), so every internal tool call shares the same
    /// peer-IP bucket as user traffic and a rapid agent tool loop makes the
    /// server self-throttle (HTTP 429 "Too Many Requests"). When this is
    /// `false` the `GovernorLayer` is not installed at all, so NO traffic —
    /// internal or external — is rate-limited.
    #[serde(default = "default_rate_limit_enabled")]
    pub enabled: bool,
    /// Sustained requests-per-second per peer IP.
    #[serde(default = "default_rate_limit_per_second")]
    pub per_second: u64,
    /// Token-bucket burst capacity.
    #[serde(default = "default_rate_limit_burst_size")]
    pub burst_size: u32,
}

fn default_rate_limit_enabled() -> bool {
    true
}

fn default_rate_limit_per_second() -> u64 {
    5
}

fn default_rate_limit_burst_size() -> u32 {
    60
}

#[derive(Debug, Deserialize, Clone)]
pub struct CorsConfig {
    pub allow_origins: Vec<String>,
    pub allow_methods: Vec<String>,
    pub allow_headers: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub issuer: String,
    pub audience: String,
    pub access_token_expiry_hours: i64,
    #[serde(default = "default_refresh_token_expiry")]
    pub refresh_token_expiry_days: i64,
}

fn default_refresh_token_expiry() -> i64 {
    30
}

impl Config {
    pub fn load_from(
        config_path: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Get config file path from parameter or environment variable
        let config_path = config_path
            .or_else(|| std::env::var("CONFIG_FILE").ok())
            .ok_or("Config file path not provided. Use --config-file argument or set CONFIG_FILE environment variable (e.g., CONFIG_FILE=config/dev.yaml)")?;

        tracing::info!("Loading configuration from: {}", config_path);

        // Read the file
        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file '{}': {}", config_path, e))?;

        // Parse YAML
        let mut config: Config = serde_yaml::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config file '{}': {}", config_path, e))?;

        // Validate configuration
        if config.postgresql.use_embedded && config.postgresql.embedded.is_none() {
            return Err("use_embedded is true but embedded configuration is missing".into());
        }
        if !config.postgresql.use_embedded && config.postgresql.external.is_none() {
            return Err("use_embedded is false but external configuration is missing".into());
        }

        // Handle automatic port assignment if port is 0
        if config.postgresql.use_embedded
            && let Some(ref mut embedded) = config.postgresql.embedded
                && embedded.port == 0 {
                    embedded.port = find_available_port(50000, 50099)
                        .ok_or("Failed to find available port for database")?;
                    tracing::info!("Auto-assigned database port: {}", embedded.port);
                }

        if config.server.port == 0 {
            config.server.port = find_available_port(3000, 3099)
                .ok_or("Failed to find available port for server")?;
            tracing::info!("Auto-assigned server port: {}", config.server.port);
        }

        // Fill every unset path field by joining `app.data_dir` with a
        // fixed subpath. Idempotent. After this call, every Optional path
        // on the Config is `Some(...)` and callers can `.unwrap()`.
        config.resolve_paths();

        Ok(config)
    }

    /// Resolve every Optional path field by deriving from `app.data_dir`.
    /// Called once at the end of `load_from`. Idempotent: existing
    /// `Some(...)` values are preserved as-is (operator overrides win
    /// over derived defaults).
    pub fn resolve_paths(&mut self) {
        // 1. Ensure app.data_dir exists. Falls back to ~/.ziee per the
        //    same convention init_data_dir uses.
        let app_data_dir: PathBuf = match &self.app {
            Some(a) => PathBuf::from(&a.data_dir),
            None => {
                let default = dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".ziee");
                self.app = Some(AppConfig {
                    data_dir: default.to_string_lossy().into_owned(),
                });
                default
            }
        };

        // 2. postgres install + data dirs.
        if let Some(ref mut emb) = self.postgresql.embedded {
            emb.installation_dir
                .get_or_insert_with(|| join_to_string(&app_data_dir, "postgres"));
            emb.data_dir
                .get_or_insert_with(|| join_to_string(&app_data_dir, "postgres-data"));
        }

        // 3. code_sandbox paths.
        let sandbox = self.code_sandbox.get_or_insert_with(CodeSandboxConfig::default);
        sandbox
            .rootfs_path
            .get_or_insert_with(|| join_to_string(&app_data_dir, "sandbox-rootfs"));
        sandbox
            .workspace_root
            .get_or_insert_with(|| join_to_string(&app_data_dir, "sandboxes"));

        // 4. Caches (HuggingFace models, LLM engine binaries, git, LFS).
        self.caches
            .hf_models_dir
            .get_or_insert_with(|| join_to_string(&app_data_dir, "hf-models"));
        self.caches
            .llm_engines_dir
            .get_or_insert_with(|| join_to_string(&app_data_dir, "llm-engines"));
        self.caches
            .git_cache_dir
            .get_or_insert_with(|| join_to_string(&app_data_dir, "cache/git"));
        self.caches
            .lfs_cache_dir
            .get_or_insert_with(|| join_to_string(&app_data_dir, "cache/lfs"));
    }

    /// Helper for code paths that have a resolved `Config` and need the
    /// installation_dir for the embedded postgres install.
    pub fn embedded_postgres_installation_dir(&self) -> Option<PathBuf> {
        self.postgresql
            .embedded
            .as_ref()
            .and_then(|e| e.installation_dir.as_ref())
            .map(PathBuf::from)
    }

    pub fn database_url(&self) -> String {
        if self.postgresql.use_embedded {
            let embedded = self
                .postgresql
                .embedded
                .as_ref()
                .expect("embedded config must be present when use_embedded is true");
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                embedded.username,
                embedded.password,
                embedded.bind_address,
                embedded.port,
                embedded.database
            )
        } else {
            let external = self
                .postgresql
                .external
                .as_ref()
                .expect("external config must be present when use_embedded is false");
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                external.username,
                external.password,
                external.host,
                external.port,
                external.database
            )
        }
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}

/// Join a subpath onto a base dir and stringify. Used by `resolve_paths`
/// to fill Option<String> path defaults. `to_string_lossy` is fine here:
/// `app.data_dir` originates from the YAML config (UTF-8) or our
/// `~/.ziee` default (ASCII), neither of which produce surrogate halves.
fn join_to_string(base: &std::path::Path, sub: &str) -> String {
    base.join(sub).to_string_lossy().into_owned()
}

/// Find an available port in the given range
fn find_available_port(start_port: u16, end_port: u16) -> Option<u16> {
    use std::net::{SocketAddr, TcpListener};

    for port in start_port..=end_port {
        if let Ok(listener) = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))) {
            drop(listener);
            // Double-check with a second attempt
            if let Ok(listener2) = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))) {
                drop(listener2);
                return Some(port);
            }
        }
    }

    // Fallback to portpicker if range is exhausted
    portpicker::pick_unused_port()
}

#[cfg(test)]
mod rate_limit_config_tests {
    use super::RateLimitConfig;

    // serde_json (not yaml) keeps the test dependency-free; RateLimitConfig
    // derives Deserialize so the format is irrelevant to what we assert.

    #[test]
    fn enabled_defaults_true_when_field_omitted() {
        // The pre-existing block shape (per_second/burst_size only, e.g.
        // tests/common/mod.rs) must keep the limiter enabled.
        let cfg: RateLimitConfig =
            serde_json::from_str(r#"{"per_second":1,"burst_size":2}"#).unwrap();
        assert!(cfg.enabled, "enabled should default to true");
        assert_eq!(cfg.per_second, 1);
        assert_eq!(cfg.burst_size, 2);
    }

    #[test]
    fn can_disable_with_just_enabled_flag() {
        // `enabled: false` alone is enough; per_second/burst_size fall back
        // to their serde defaults.
        let cfg: RateLimitConfig = serde_json::from_str(r#"{"enabled":false}"#).unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.per_second, 5);
        assert_eq!(cfg.burst_size, 60);
    }

    #[test]
    fn full_block_parses_all_fields() {
        let cfg: RateLimitConfig =
            serde_json::from_str(r#"{"enabled":true,"per_second":100,"burst_size":200}"#).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.per_second, 100);
        assert_eq!(cfg.burst_size, 200);
    }
}
