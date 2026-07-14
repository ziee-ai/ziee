use serde::Deserialize;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

// The app-agnostic server settings — postgresql / server (host/port/CORS/
// rate-limit) / logging / jwt — moved to `ziee_core::config::ServerConfig` in
// Chunk B2 (the Config split). ziee's monolithic `Config` composes it via
// `#[serde(flatten)]` + `Deref`, so the serialized (YAML) shape is byte-identical
// and every `config.postgresql` / `config.server` / `config.jwt` /
// `config.database_url()` call site keeps working unchanged. These types are
// re-exported so the many `crate::core::config::{JwtConfig, CorsConfig, …}` and
// `ziee::{CorsConfig, JwtConfig}` paths resolve exactly as before. The full set
// (not just the internally-referenced ones) is re-exported to preserve the
// pre-split public surface of `crate::core::config`.
#[allow(unused_imports)]
pub use ziee_core::config::{
    CorsConfig, EmbeddedPostgreSqlConfig, ExternalPostgreSqlConfig, HttpServerConfig, JwtConfig,
    LoggingConfig, LoggingConfigPostgres, PoolConfig, PostgreSqlConfig, RateLimitConfig,
    ServerConfig,
};

// Chunk BA-full: the `From<JwtConfig> for JwtSettings` bridge moved INTO
// `ziee-auth` (`auth::jwt`) — once `JwtSettings` moved to the SDK crate, both
// types are foreign to the app, so the impl can no longer live here (orphan
// rule). `ziee-auth` depends on `ziee-core` (which owns `JwtConfig`), so it
// owns the conversion. Every `JwtService::try_new(config.jwt.into())` call site
// is unchanged.

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Framework server settings (postgresql / server / logging / jwt),
    /// flattened so the wire shape is byte-identical to the pre-split Config.
    /// `Deref`/`DerefMut` expose its fields directly, so `config.postgresql`,
    /// `config.server`, `config.jwt`, `config.database_url()`, etc. are
    /// unchanged.
    #[serde(flatten)]
    pub server_config: ServerConfig,

    #[serde(default)]
    pub app: Option<AppConfig>,
    #[serde(default)]
    pub code_sandbox: Option<CodeSandboxConfig>,
    #[serde(default)]
    pub bio_mcp: Option<BioMcpConfig>,
    #[serde(default)]
    pub lit_search: Option<LitSearchConfig>,
    #[serde(default)]
    pub web_search: Option<WebSearchConfig>,

    /// Voice dictation (managed whisper.cpp speech-to-text runtime). Absent =
    /// enabled. Deploy-level kill switch: `voice: { enabled: false }`.
    #[serde(default)]
    pub voice: Option<VoiceConfig>,
    #[serde(default)]
    pub control_mcp: Option<ControlMcpConfig>,
    #[serde(default)]
    pub js_tool: Option<JsToolConfig>,
    #[serde(default)]
    pub secrets: Option<SecretsConfig>,
    /// Per-cache path overrides. Defaults to all-None; `Config::resolve_paths`
    /// fills each unset field with a subdir of `app.data_dir`. Operators
    /// override individual entries to put a particular cache on a
    /// different disk (e.g. `hf_models_dir` on a big spinning disk while
    /// `git_cache_dir` stays on the SSD).
    #[serde(default)]
    pub caches: CachesConfig,
    /// Daily check against the GitHub Releases API for a newer `ziee`.
    /// NOTIFICATION ONLY — never downloads or installs. Defaults to enabled;
    /// air-gapped operators set `update_check: { enabled: false }` to suppress
    /// all outbound calls + the admin update banner. Forced off in the embedded
    /// desktop server (the desktop app has its own auto-updater).
    #[serde(default)]
    pub update_check: UpdateCheckConfig,
}

// Transparent access to the flattened `ServerConfig`: `config.postgresql`,
// `config.server`, `config.jwt`, `config.logging`, `config.database_url()`,
// `config.server_address()` all resolve through `Deref`/`DerefMut` exactly as
// they did when these were inline fields/methods on `Config`.
impl Deref for Config {
    type Target = ServerConfig;
    fn deref(&self) -> &ServerConfig {
        &self.server_config
    }
}

impl DerefMut for Config {
    fn deref_mut(&mut self) -> &mut ServerConfig {
        &mut self.server_config
    }
}

/// Server self-update notification config. See `Config::update_check`.
#[derive(Debug, Deserialize, Clone)]
pub struct UpdateCheckConfig {
    #[serde(default = "default_update_check_enabled")]
    pub enabled: bool,
}

impl Default for UpdateCheckConfig {
    fn default() -> Self {
        Self {
            enabled: default_update_check_enabled(),
        }
    }
}

fn default_update_check_enabled() -> bool {
    true
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

// `CodeSandboxConfig` moved to the build-DB-free sandbox engine crate
// (`ziee_sandbox::sandbox_config`) with the engine carve; re-exported here so
// this crate's `Config { code_sandbox: Option<CodeSandboxConfig> }` field +
// `resolve_paths` field access + the `public_file_origin`/`rootfs_path`/
// `workspace_root` call sites resolve unchanged.
pub use ziee_sandbox::sandbox_config::CodeSandboxConfig;

/// Configuration for the `bio_mcp` built-in MCP server (BioMCP biomedical
/// connectors run as a managed `biomcp serve-http` sidecar).
///
/// Connected-only: the sidecar queries live upstream APIs (PubMed,
/// ClinicalTrials.gov, …). **On by default** (per the feature roadmap) for
/// connected deployments — the module self-disables when the embedded
/// binary is a build stub or the host is offline. IP-sensitive operators
/// turn it off with `bio_mcp: { enabled: false }`, since query terms
/// egress to public APIs. This is the deploy-level kill switch; the
/// per-deployment admin runtime toggle is the `mcp_servers.enabled`
/// column on the bio row.
#[derive(Debug, Deserialize, Clone)]
pub struct BioMcpConfig {
    /// Master switch. When false, the module's `init()` returns early
    /// (no MCP row upsert, no sidecar ever spawned). Defaults to true.
    #[serde(default = "default_bio_mcp_enabled")]
    pub enabled: bool,
}

fn default_bio_mcp_enabled() -> bool {
    true
}

impl Default for BioMcpConfig {
    fn default() -> Self {
        Self {
            enabled: default_bio_mcp_enabled(),
        }
    }
}

/// Configuration for the `lit_search` built-in MCP server (live scholarly
/// literature search + open-access full-text fetch).
///
/// Connected-only: the connectors query live public APIs (Europe PMC,
/// Crossref, Semantic Scholar, PubMed, arXiv, CORE), so **query terms egress**.
/// On by default for connected deployments. IP-sensitive operators turn it off
/// with `lit_search: { enabled: false }` — a **deploy-level** kill switch that
/// an admin cannot re-enable (distinct from the runtime admin toggle, the
/// `lit_search_settings.enabled` row). When false, `init()` returns before the
/// MCP row upsert, so the tools are never registered.
#[derive(Debug, Deserialize, Clone)]
pub struct LitSearchConfig {
    /// Master switch. When false, the module's `init()` returns early (no MCP
    /// row upsert). Defaults to true.
    #[serde(default = "default_lit_search_enabled")]
    pub enabled: bool,
}

fn default_lit_search_enabled() -> bool {
    true
}

impl Default for LitSearchConfig {
    fn default() -> Self {
        Self {
            enabled: default_lit_search_enabled(),
        }
    }
}

/// Configuration for the `js_tool` built-in (`run_js` programmatic tool calling).
/// The embedded QuickJS interpreter runs IN-PROCESS with zero ambient capability
/// and only exposes tools the conversation already has (mutating sub-tools still
/// require per-call approval), so it is on by default. A deploy-level operator
/// turns it off with `js_tool: { enabled: false }` — a kill switch an admin
/// cannot re-enable. When false, the chat extension never sets the attach flag,
/// so `run_js` is never offered to any model.
#[derive(Debug, Deserialize, Clone)]
pub struct JsToolConfig {
    /// Master switch. When false, `run_js` is never attached. Defaults to true.
    #[serde(default = "default_js_tool_enabled")]
    pub enabled: bool,
}

fn default_js_tool_enabled() -> bool {
    true
}

impl Default for JsToolConfig {
    fn default() -> Self {
        Self {
            enabled: default_js_tool_enabled(),
        }
    }
}

/// Configuration for the `web_search` built-in MCP server (web search + page
/// fetch). Connected-only: query terms egress to the configured search
/// provider, so IP-sensitive operators turn it off with
/// `web_search: { enabled: false }` — a **deploy-level** kill switch an admin
/// cannot re-enable (distinct from the runtime `web_search_settings.enabled`
/// row). When false, `init()` returns before the MCP row upsert, so the tools
/// are never registered. Mirrors [`LitSearchConfig`].
#[derive(Debug, Deserialize, Clone)]
pub struct WebSearchConfig {
    /// Master switch. When false, the module's `init()` returns early (no MCP
    /// row upsert). Defaults to true.
    #[serde(default = "default_web_search_enabled")]
    pub enabled: bool,
}

fn default_web_search_enabled() -> bool {
    true
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            enabled: default_web_search_enabled(),
        }
    }
}

/// Configuration for the `voice` dictation runtime (managed whisper.cpp
/// speech-to-text). Fully local — no cloud STT. `voice: { enabled: false }` is a
/// **deploy-level** kill switch an admin cannot re-enable (distinct from the
/// runtime `voice_runtime_settings.enabled` toggle). When false, `init()`
/// returns before spawning the reaper / registering surfaces.
#[derive(Debug, Deserialize, Clone)]
pub struct VoiceConfig {
    /// Master switch. When false, the module's `init()` returns early. Defaults
    /// to true.
    #[serde(default = "default_voice_enabled")]
    pub enabled: bool,
}

fn default_voice_enabled() -> bool {
    true
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: default_voice_enabled(),
        }
    }
}

/// Configuration for the `control_mcp` built-in MCP server (app-control tools
/// that let the chat model operate ziee's own REST API). Enabled for everyone by
/// default. Operators disable the WHOLE control surface with
/// `control_mcp: { enabled: false }` — a **deploy-level** kill switch (§16).
/// When false, `init()` returns before the MCP row upsert and `register_routes`
/// skips the endpoint, so the tools are never registered.
#[derive(Debug, Deserialize, Clone)]
pub struct ControlMcpConfig {
    /// Master switch. When false, the module's `init()` returns early (no MCP
    /// row upsert) and the route is not registered. Defaults to true.
    #[serde(default = "default_control_mcp_enabled")]
    pub enabled: bool,
}

fn default_control_mcp_enabled() -> bool {
    true
}

impl Default for ControlMcpConfig {
    fn default() -> Self {
        Self {
            enabled: default_control_mcp_enabled(),
        }
    }
}

impl CachesConfig {
    #[allow(dead_code)]
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

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub data_dir: String,
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
        let mut config: Config = serde_norway::from_str(&config_content)
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
    #[allow(dead_code)]
    pub fn embedded_postgres_installation_dir(&self) -> Option<PathBuf> {
        self.postgresql
            .embedded
            .as_ref()
            .and_then(|e| e.installation_dir.as_ref())
            .map(PathBuf::from)
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
mod voice_config_tests {
    use super::VoiceConfig;

    // serde_json keeps the test dependency-free; VoiceConfig derives Deserialize
    // so the wire format is irrelevant to what we assert. Mirrors how the module
    // resolves the deploy-level kill switch:
    //   config.voice.as_ref().map(|c| c.enabled).unwrap_or(true)   (voice/mod.rs)

    /// The exact gate the module applies: an absent `voice:` section (None) means
    /// ENABLED; only an explicit `enabled: false` disables it.
    fn resolve(cfg: Option<VoiceConfig>) -> bool {
        cfg.as_ref().map(|c| c.enabled).unwrap_or(true)
    }

    #[test]
    fn absent_voice_section_defaults_to_enabled() {
        assert!(resolve(None), "an absent voice: config must default to enabled");
        // And an empty block `voice: {}` (present but no fields) is also enabled.
        let empty: VoiceConfig = serde_json::from_str("{}").unwrap();
        assert!(empty.enabled);
        assert!(resolve(Some(empty)));
    }

    #[test]
    fn explicit_enabled_false_disables() {
        let cfg: VoiceConfig = serde_json::from_str(r#"{"enabled":false}"#).unwrap();
        assert!(!cfg.enabled);
        assert!(!resolve(Some(cfg)), "voice enabled:false must disable");

        let on: VoiceConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(on.enabled);
        assert!(resolve(Some(on)));
    }
}

#[cfg(test)]
mod packaging_config_tests {
    use super::Config;

    /// The default config shipped in the .deb/.rpm/.apk packages
    /// (`packaging/config.default.yaml`) is what systemd boots from on a clean
    /// install — it MUST parse as a full `Config` (e.g. embedded Postgres needs
    /// its non-optional `logging` sub-block, or the service crash-loops).
    #[test]
    fn packaged_default_config_parses() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packaging/config.default.yaml"
        );
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {path}: {e}"));
        serde_norway::from_str::<Config>(&content).unwrap_or_else(|e| {
            panic!(
                "packaging/config.default.yaml must parse as Config (a clean \
                 package install boots from it): {e}"
            )
        });
    }
}
