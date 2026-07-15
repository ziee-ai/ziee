// Core modules for modular architecture
mod common;
mod core;
mod module_api;
mod modules;
mod openapi;
mod utils;

// Core macros moved to `ziee-core` in Chunk B1; re-exported at the crate root
// so existing `crate::sse_event_enum!` / `crate::impl_string_to_enum!` /
// `crate::impl_json_from!` call sites resolve unchanged (decision N2).
pub use ziee_core::{impl_json_from, impl_string_to_enum, sse_event_enum};

/// Rust port of the former `ui/openapi/generate-endpoints.ts`. Re-exported so
/// the desktop crate can emit its own `types.ts` from the combined OpenAPI spec
/// without the Node/tsx codegen step.
pub use ziee_framework::openapi::emit_ts::generate_types_ts_from_json;

/// Chunk B6: the app-agnostic OpenAPI emit TAIL (finish_api → openapi.json →
/// emit_ts → types.ts, output paths parameterized), moved to `ziee-framework`.
/// Re-exported at the crate root so the desktop crate can drive it with the
/// combined (server + desktop) router + its own output paths.
pub use ziee_framework::openapi::finish_and_emit;

use module_api::ModuleContext;
use std::net::SocketAddr;
use std::sync::Arc;

// Re-export types for desktop/external use
pub use core::config::{Config, CorsConfig, JwtConfig};
pub use core::{Repos, EventBus, EventHandler, AppEvent};
// Chunk BG-3: the desktop-consumer boot path (ziee-desktop's `ServerBoot` impl +
// `ensure_desktop_admin`) threads the `BootHandle.pool` into repositories rather
// than reaching the global `Repos`. `AppRepository` is the app-side owner-create
// domain CRUD (kept app-side by BA); re-exported so the desktop crate can build
// it from a threaded pool. `UserRepository` (owner read) lives in `ziee-auth` and
// is consumed via the harness single-user strategy.
pub use modules::app::AppRepository;
pub use modules::user::UserRepository;
// Re-exported so integration tests (which construct repositories directly
// against the test DB pool) can initialise the same at-rest storage_key
// that the spawned server process used. Without this, repo.get() in the
// test process can't decrypt rows the server wrote, and resolve-fallback
// returns None. See common::secret::resolve_optional_secret.
#[doc(hidden)]
pub use core::secrets::{init_storage_key, storage_key};
pub use module_api::ModuleContext as ServerContext;
pub use modules::auth::{AuthRepository, AuthResponse, JwtService, SessionSettingsRepository, hash_password};
pub use modules::auth::jwt::JwtSettings;
pub use modules::auth::jwt_extractor::JwtAuth;
pub use modules::auth::refresh_tokens;
pub use modules::user::models::User;
pub use modules::llm_provider::events::LlmProviderEvent;
pub use modules::llm_provider::UserKeyRepository;
pub use modules::mcp::events::McpServerEvent;
// Re-exported so integration tests can drive the REAL retention reaper tick
// (`memory::reaper::run_once`) instead of mirroring its SQL.
pub use modules::memory::reaper::run_once as memory_reaper_run_once;
// Re-exported so integration tests can drive the REAL elicitation_mcp built-in
// upsert (idempotency / url re-assertion) instead of mirroring its SQL.
// Re-exported so integration tests can exercise the REAL cross-tenant security
// filter (the JOIN to `files` on `user_id`) instead of mirroring its SQL.
pub use modules::llm_provider_files::repository::get_provider_file_mapping as llm_provider_file_mapping_for_user;
// Re-exported so the integration test can assert the REAL anti-injection guard
// text in the extraction prompt (replacing a no-op stub).
pub use modules::memory::engine::prompts::EXTRACTION_PROMPT as memory_extraction_prompt;
// Re-exported so an integration test can drive the REAL OAuth username-collision
// retry loop (base → base2 → base3 …) instead of going through the full OAuth flow.
pub use modules::auth::handlers::ensure_unique_username as auth_ensure_unique_username;
pub use modules::elicitation_mcp::elicitation_mcp_server_id;
pub use modules::elicitation_mcp::repository::ElicitationMcpRepository;
// Re-export the LLM repository connection-health entry points so the
// integration tests can drive the boot path directly without going
// through the module's `init` hook.
#[doc(hidden)]
pub mod llm_repository_health {
    pub use crate::modules::llm_repository::connection_health::run_startup_health_check;
}
// Re-export the memory extraction prompt so integration tests can assert its
// anti-injection / PII guards against the real runtime constant.
#[doc(hidden)]
pub mod memory_test_api {
    pub use crate::modules::memory::engine::prompts::EXTRACTION_PROMPT;
}
pub use modules::chat::core::ai_provider::resolve_api_key_for_user;
pub use common::{ApiResult, AppError};
// Re-export the at-rest secret helpers so out-of-crate consumers
// (notably the desktop tauri crate's remote_access module) can
// encrypt/decrypt rows without re-implementing pgcrypto plumbing.
pub use common::secret::{decrypt_secret, encrypt_secret, resolve_optional_secret, SecretView};
// Re-export password helpers so the desktop crate's remote_access
// module can validate + hash passwords without reaching into private
// auth internals.
pub mod password {
    pub use crate::modules::auth::password::{
        hash_password, validate_password_strength, verify_password,
    };
}
// Re-export the permissions surface so desktop modules can gate
// their HTTP handlers with `RequirePermissions<(...)>` and define
// their own `PermissionCheck` permission types.
pub mod permissions {
    pub use crate::modules::permissions::{RequirePermissions, with_permission};
    pub use crate::modules::permissions::types::{PermissionCheck, PermissionList};
}
// Re-export async_trait for consistent EventHandler implementations
pub use async_trait::async_trait;

// Re-export MCP client types for integration tests
pub use modules::mcp::client::http::{HeaderParseError, HttpMcpClient, parse_header_map};
pub use modules::mcp::client::stdio::StdioMcpClient;
pub use modules::mcp::client::auth::{
    OAuthClientConfig, StoredToken, refresh_token as oauth_refresh_token,
};
pub use modules::mcp::client::traits::McpClient;
pub use modules::mcp::{McpServer, TransportType, UsageMode};
pub use modules::mcp::sampling::handler::{ChatSamplingHandler, SamplingHandler};
pub use modules::mcp::sampling::models::{
    SamplingContent, SamplingCreateMessageRequest, SamplingCreateMessageResult,
};
pub use ai_providers::Provider as AiProvider;

// Re-export elicitation primitives for integration tests.
#[doc(hidden)]
pub use modules::mcp::elicitation::models::{
    ElicitationResponse, ElicitationStartedNotification,
};
#[doc(hidden)]
pub use modules::mcp::elicitation::registry as elicitation_registry;

// Re-export the embedded mac-sandbox-runtime accessor for the
// self-contained verification tests. The whole module is mac-arm64-only
// useful but the symbol exists on every target (empty BUNDLE elsewhere).
#[doc(hidden)]
pub use modules::code_sandbox::embedded as code_sandbox_embedded;

// Re-export app_data_dir setter so the selective-wipe regression test
// can redirect the app data root to a TempDir without going through
// the full config-load path.
#[doc(hidden)]
pub use core::{set_app_data_dir, set_caches_config};

// Test-helper exports: the platform-specific sandbox backend dispatch
// + the raw-exec result shape. Used by tests/code_sandbox/harness.rs
// to drive tier-4 hardening tests through the same backend the prod
// code uses (instead of `Command::new("bwrap")` directly).
#[doc(hidden)]
pub use modules::code_sandbox::backend::{active as sandbox_backend, RawExecResult};

// Re-export MCP content types for integration tests
#[doc(hidden)]
pub use modules::mcp::chat_extension::content::{McpContentData, ResourceLink, RichFile};

// Re-export the shared resource_link consumer for integration tests (the
// in-process callers — chat extension + workflow dispatcher — reach it via
// `crate::modules::mcp::resource_link`, but `modules` is private to the lib).
// `init_file_storage` lets an in-process test point the global file store at a temp
// dir before driving `persist_links` directly (the spawned test server inits its own
// store in its own process; `init_repositories` is already re-exported above).
#[doc(hidden)]
pub use modules::file::storage::manager::{get_file_storage, init_file_storage};
#[doc(hidden)]
pub use modules::mcp::resource_link::{
    persist_links, result_link_trusted_hosts, PersistOutcome, PersistedArtifact,
};

// Re-export memory + summarization engines for integration tests
// (tier 5 real-LLM tests need to invoke the pipelines directly).
#[doc(hidden)]
pub mod memory_extensions {
    pub use crate::modules::memory::engine::extractor;
}

#[doc(hidden)]
pub mod summarization_engine {
    pub use crate::modules::summarization::engine::summarizer;
}

// Re-export code_sandbox surface for integration tests (tier 2 + 3).
#[doc(hidden)]
pub mod code_sandbox {
    pub use crate::modules::code_sandbox::{
        code_sandbox_server_id, loopback_host, CodeSandboxRepository,
    };
    // Generic mount-provider seam (feature #3, Part B0). The desktop crate's
    // `host_mount` module registers a provider here at boot to inject host
    // folder mounts without the server core knowing about host folders.
    pub use crate::modules::code_sandbox::mount_provider::{
        has_providers, register_sandbox_mount_provider, MountSpec, SandboxMountProvider,
    };
    pub use crate::modules::code_sandbox::types::SandboxContext;
    pub use crate::modules::code_sandbox::workflow_staging::StageMode;
}
// Re-export elicitation_mcp surface for integration tests (built-in row
// idempotency).
#[doc(hidden)]
pub mod elicitation_mcp {
    pub use crate::modules::elicitation_mcp::elicitation_mcp_server_id;
    pub use crate::modules::elicitation_mcp::repository::ElicitationMcpRepository;
}
// Re-export the SSO auto-provision username-uniqueness helper for integration
// tests (collision-suffix + empty-base default).
#[doc(hidden)]
pub use modules::auth::handlers::ensure_unique_username;
// MCP repository for integration tests that need McpRepository::list_accessible.
#[doc(hidden)]
pub mod mcp {
    pub use crate::modules::mcp::McpRepository;
}

// Re-export the workflow_mcp built-in-server registration surface for the
// Tier-2 repository idempotency tests (mirrors the `code_sandbox` block above:
// `ziee::code_sandbox::CodeSandboxRepository`).
#[doc(hidden)]
pub mod workflow_mcp {
    pub use crate::modules::workflow_mcp::repository::WorkflowMcpRepository;
    pub use crate::modules::workflow_mcp::workflow_mcp_server_id;
}

// Re-export the web_search built-in-server registration surface for the Tier-2
// restart/re-register idempotency test (mirrors the workflow_mcp block above).
#[doc(hidden)]
pub mod web_search {
    pub use crate::modules::web_search::{web_search_server_id, WebSearchRepository};
}

// Re-export the bio_mcp supervisor shutdown for the sidecar death+respawn
// recovery test (kills the running sidecar so the next call must respawn).
#[doc(hidden)]
pub mod bio_mcp {
    pub use crate::modules::bio_mcp::supervisor;
}

// Re-export the llm_provider_files service + the file storage/repo + provider
// model surface for the re-upload-after-failure integration test (audit
// 880298cae9cb): drives the REAL get_or_upload_provider_file against a real
// FilesystemStorage blob + a mock AIProvider that fails-then-succeeds.
#[doc(hidden)]
pub mod llm_provider_files_test_api {
    pub use crate::modules::file::storage::filesystem::FilesystemStorage;
    pub use crate::modules::file::storage::FileStorage;
    pub use crate::modules::file::FileRepository;
    pub use crate::modules::llm_provider::models::{LlmProvider, ProxySettings};
    pub use crate::modules::llm_provider_files::repository::get_provider_file_mapping;
    pub use crate::modules::llm_provider_files::service::get_or_upload_provider_file;
}

// Re-export the memory chat-extension retrieval+injection entrypoint for the
// integration test that exercises the combined recall + core-memory injection
// flow against a real assistant.
#[doc(hidden)]
pub mod memory {
    pub use crate::modules::memory::chat_extension::retriever::retrieve_and_inject;
}

// Re-export the available-files resolver + type for the integration test that
// exercises checksum dedup through the REAL upload+attach flow.
#[doc(hidden)]
pub mod file_available {
    pub use crate::modules::file::available_files::{resolve_available_files, AvailableFile};
}

// Re-export the provider file-routing entrypoint for the integration test that
// exercises its ownership re-validation + routing dispatch.
#[doc(hidden)]
pub mod file_routing {
    pub use crate::modules::file::provider_routing::process_file_blocks;
}

// Re-export the file_rag reindex entrypoint for the integration test that
// exercises the per-file advisory-lock under concurrent re-ingest.
#[doc(hidden)]
pub mod file_rag_ingest {
    pub use crate::modules::file_rag::ingest::reindex_file;
}

// Re-export the workflow_mcp await-terminal loop for the crashed-runner
// (no-progress guard) integration test.
#[doc(hidden)]
pub mod workflow_mcp_internal {
    pub use crate::modules::workflow_mcp::tools::await_terminal_for_test;
}

// Re-export file_rag search arms for the concurrent-search-during-embed race
// test (NULL embeddings mid-rebuild → vector arm excludes them, FTS still serves).
#[doc(hidden)]
pub mod file_rag_search {
    pub use crate::modules::file_rag::retrieval::{
        fts_search_hit_count_for_test, vector_search_hit_count_for_test,
    };
}

// Re-export the workflow run-status-machine surface for the Tier-2 status-
// machine tests (D1–D5): the REAL `mark_status` CAS, the `mark_running` /
// `cancel_cas` / `heartbeat` per-transition guards, the `persist_step_meta`
// jsonb-merge, and a chrono-free wrapper over the startup-sweep orphan flip —
// so the tests exercise the real fns rather than transcribed SQL.
#[doc(hidden)]
pub mod workflow {
    pub use crate::modules::workflow::models::WorkflowRunStatus;
    pub use crate::modules::workflow::repository::{
        cancel_cas, heartbeat, insert_run, mark_running, mark_status, persist_step_meta,
    };
    pub use crate::modules::workflow::models::CreateWorkflowRun;
    // The run staging root, so a test can delete a run's on-disk logs to
    // exercise read_log's durable step_logs_json fallback (A7 GC recovery).
    pub use crate::modules::workflow::runner::workflow_workspace_root;

    /// Test-only wrapper over `repository::fail_orphaned_runs` taking the cutoff
    /// as unix-epoch seconds, so the integration-test crate (which has no `time`
    /// crate in scope) can drive the real orphan-flip without naming
    /// `time::OffsetDateTime`.
    pub async fn fail_orphaned_runs_before_unix(
        pool: &sqlx::PgPool,
        cutoff_unix_secs: i64,
    ) -> Result<u64, crate::common::AppError> {
        let cutoff = time::OffsetDateTime::from_unix_timestamp(cutoff_unix_secs)
            .expect("valid unix timestamp");
        crate::modules::workflow::repository::fail_orphaned_runs(pool, cutoff).await
    }
}

// Private pure helpers that integration tests unit-test directly (wire-format
// grouping). Kept out of the public docs; the ai_providers
// wire types are re-exported too because that crate is a dependency of `ziee` but
// not of the integration-test crate.
#[doc(hidden)]
pub mod test_internals {
    pub use crate::modules::chat::core::services::streaming::{
        dedup_tool_results_by_id, group_assistant_blocks,
    };
    pub use ai_providers::{ChatMessage, ContentBlock, Role};
    // Chat repository surface for the DB-level append_content tests
    // (Tier-2 monotonic / collision-free under concurrent appends).
    pub use crate::modules::chat::core::repository::ChatCoreRepository;
    pub use crate::modules::chat::core::models::MessageContentData;
    // Local-runtime proxy token surface, so integration tests can drive the
    // boot-time reseed (which mints + persists a proxy token for keyless local
    // providers) and assert against the in-memory token cache.
    pub use crate::modules::llm_local_runtime::proxy::{
        clear_cache as proxy_clear_cache, lookup_token as proxy_lookup_token,
        reseed_from_db as proxy_reseed_from_db,
    };
    // `resolve_optional_secret` is already re-exported at the crate root
    // (see the `pub use common::secret::{...}` above) — tests use that path.
}

// Re-export axum types for route building
pub use axum::{Extension, Json, extract::State, http::StatusCode};
pub use axum::routing::{get, post};
pub use axum::Router;

// Re-export aide types for route building with OpenAPI
pub use aide::axum::ApiRouter;
pub use aide::axum::routing::{get_with, post_with, put_with, delete_with};
pub use aide::transform::TransformOperation;

// Re-export app_builder functions for desktop OpenAPI generation
// and for desktop crates that need to re-apply CORS / security-header
// layers to their own merged-in routes (axum's `.merge()` does NOT
// propagate parent layers onto merged routes).
pub use core::app_builder::{create_cors_layer, create_modules, build_api_router, initialize_modules};
pub use core::database::initialize_database;
pub use core::{init_repositories, is_repos_initialized};
pub use module_api::AppModule;

/// Server setup result containing components needed for customization
struct ServerSetup {
    app: Router,
    jwt_service: Arc<JwtService>,
    addr: String,
}

/// Initialize tracing based on config. Closes 14-core F-23 (Info):
/// uses EnvFilter so operators can do `RUST_LOG=ziee=debug,sqlx=warn`
/// for module-level filtering. Falls back to the config-file level when
/// RUST_LOG is unset.
fn init_tracing(config: &Config) {
    use tracing_subscriber::filter::EnvFilter;
    let config_level = config
        .logging
        .as_ref()
        .map(|l| l.level.as_str())
        .unwrap_or("info");
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config_level));

    let format = config
        .logging
        .as_ref()
        .map(|l| l.format.as_str())
        .unwrap_or("default");
    match format {
        "compact" => {
            tracing_subscriber::fmt()
                .compact()
                .with_env_filter(env_filter)
                .try_init()
                .ok();
        }
        "pretty" => {
            tracing_subscriber::fmt()
                .pretty()
                .with_env_filter(env_filter)
                .try_init()
                .ok();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .try_init()
                .ok();
        }
    }
}

/// Initialize app data directory + caches config from the resolved
/// `Config`. Called from both the standalone server entrypoint and the
/// Tauri desktop's setup_server path. `Config::resolve_paths` (run at
/// `Config::load_from`) guarantees `app.data_dir` is set and every
/// `caches.*_dir` field is `Some(...)`.
fn init_data_dir(config: &Config) {
    if let Some(ref app_config) = config.app {
        let data_dir = std::path::PathBuf::from(&app_config.data_dir);
        core::set_app_data_dir(data_dir);
    } else {
        let default_data_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".ziee");
        core::set_app_data_dir(default_data_dir);
    }
    // Publish the resolved caches config into global state so handlers
    // can read paths without threading Config through every signature.
    core::set_caches_config(config.caches.clone());
    // Apply the deployment-config chat-token SSE connection caps (DEC-34).
    crate::modules::chat::stream::registry::apply_config_limits(&config.chat);
}

/// Common server setup - initializes all components and returns the router
async fn setup_server(
    config: Config,
    additional_handlers: Vec<Arc<dyn core::EventHandler>>,
) -> Result<ServerSetup, Box<dyn std::error::Error + Send + Sync>> {
    // Initialize database
    let pool = core::database::initialize_database(&config).await?;
    tracing::info!("Database initialized with {} connections", pool.num_idle());

    // Initialize global repository factory
    core::init_repositories((*pool).clone());
    tracing::info!("Global repository factory initialized");

    // Initialize at-rest secret storage key — see core::secrets.
    core::secrets::init_storage_key(
        config
            .secrets
            .as_ref()
            .and_then(|s| s.storage_key.clone()),
    );

    // Initialize modules
    // The framework `ModuleContext` carries the app-agnostic `ServerConfig`;
    // the full monolithic `Config` is injected through the opaque `app_config`
    // slot (modules recover it via `module_api::app_config(ctx)`).
    let module_context = ModuleContext::new(
        pool.clone(),
        Arc::new(config.server_config.clone()),
        Arc::new(config.clone()),
    );
    let mut modules = core::app_builder::create_modules();

    // Initialize all modules
    core::app_builder::initialize_modules(&mut modules, &module_context)?;

    // Register event handlers from all modules
    let mut event_bus = core::app_builder::register_event_handlers(
        &modules,
        pool.clone(),
    );

    // Register additional handlers (e.g., from desktop app)
    for handler in additional_handlers {
        tracing::info!(
            "Registering additional event handler: {}",
            handler.handler_name()
        );
        event_bus.register(handler);
    }

    let event_bus = Arc::new(event_bus);
    tracing::info!(
        "Event bus initialized with {} handlers",
        event_bus.handler_count()
    );

    // Setup CORS from config
    let cors = core::app_builder::create_cors_layer(&config);

    // Set up JWT service. try_new refuses weak/placeholder secrets so
    // the server never boots with a known signer. Closes 01-auth F-10
    // + 14-core F-03.
    let jwt_service = Arc::new(
        modules::auth::JwtService::try_new(config.jwt.clone())
            .map_err(|e| {
                tracing::error!("Failed to initialize JWT service: {}", e);
                e
            })?,
    );
    tracing::info!("JWT service initialized");

    // Capture the per-file upload cap (bytes) before building the router, so the
    // upload routes' per-route body-limit layer and the upload handler share one
    // source of truth. (main.rs sets this too; setup_server sets it here so every
    // caller — incl. desktop — honors the configured cap regardless of ordering.)
    core::set_max_file_upload_bytes(
        (config.server.max_file_upload_mb as usize).saturating_mul(1024 * 1024),
    );

    // Build API router with all module routes
    let (api_router, mut api_doc) = core::app_builder::build_api_router(
        &modules,
        &config.server.api_prefix,
        (*module_context.db_pool).clone(),
    );

    // Convert ApiRouter to Router and add layers.
    //
    // SECURITY: matches the middleware stack in main.rs::main —
    // 16 MB body limit, 60s timeout, security headers, CORS.
    // Closes 14-core F-01 + 05-file F-09 generalization + A3 headers.
    //
    // Rate limiter is OPTIONAL on this embedded path. The desktop app embeds
    // this server and serves only its own local webview over 127.0.0.1 — there
    // is no per-peer-IP attack surface, and a limiter actively gets in the way
    // of legitimate burst traffic (chat streams, SSE, multi-file uploads). So
    // we pass `None` as the absent-default: when `server.rate_limit` is omitted
    // (the desktop config omits it) the layer is skipped entirely; an explicit
    // `enabled: false` also skips it. Web deployments set it explicitly (see
    // config/prod.example.yaml). See core::app_builder::apply_rate_limit_layer.
    let app = api_router
        .finish_api(&mut api_doc)
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
        // 660s — MUST exceed auto_start_timeout_secs ceiling (600s).
        // The local-runtime proxy (/api/local-llm/v1/*) waits for
        // engine auto-start synchronously before returning a
        // Response, so this layer caps the whole spawn + first-byte
        // window. See main.rs for the full rationale.
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(660),
        ));
    // Build the control MCP catalog from the now-fully-populated OpenAPI doc
    // (embedded/desktop bootstrap path — mirrors main.rs). Skipped when the
    // deploy kill-switch is off (§16).
    if config.control_mcp.as_ref().map(|c| c.enabled).unwrap_or(true) {
        crate::modules::control_mcp::catalog::init_from_openapi(&api_doc);
    }
    let app = core::app_builder::apply_rate_limit_layer(app, &config, None);
    let app = app
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-content-type-options"),
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-frame-options"),
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("referrer-policy"),
            axum::http::HeaderValue::from_static("no-referrer"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("permissions-policy"),
            axum::http::HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("strict-transport-security"),
            axum::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        // Chunk BG: the per-request auth/user dependency handle — pool + the
        // installed event/sync/outbound sinks — so those handlers no longer
        // reach `Repos` / `EventBus` / `sync::publish` / `url_validator`.
        .layer(axum::Extension(crate::core::events::build_auth_context(
            pool.clone(),
            event_bus.clone(),
        )))
        .layer(axum::Extension(event_bus))
        .layer(axum::Extension(jwt_service.clone()))
        // Chunk `ziee-file-http`: the per-request handle the mountable
        // `ziee_file::http::file_routes` handlers pull from the extensions —
        // the file store repository + ziee's `FileEvents` seam impl + the
        // download-token signer — so those handlers no longer reach `Repos.file`
        // / the file-module JWT global / `sync::publish`.
        .layer(axum::Extension(crate::modules::file::ingest::build_file_context(
            pool.clone(),
            &config.jwt,
        )))
        // Chunk B3: the framework's permission extractors pull this injected
        // resolver (backed by Repos + the JWT service above) from the request
        // extensions to authenticate + authorize, so enforcement stays generic.
        .layer(axum::Extension(Arc::new(
            crate::modules::permissions::extractors::ZieeIdentityResolver,
        )))
        .layer(cors);

    let addr = config.server_address();

    Ok(ServerSetup {
        app,
        jwt_service,
        addr,
    })
}

/// Start the server with the given router
async fn run_server(
    app: Router,
    addr: String,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let local_addr = listener.local_addr()?;

    tracing::info!(
        "ziee backend server started successfully on {}",
        local_addr
    );

    tokio::spawn(async move {
        // into_make_service_with_connect_info surfaces the TCP peer
        // address for tower_governor's PeerIpKeyExtractor — same fix
        // as main.rs. Without it, rate-limited requests return 500.
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
            .await
            .expect("Failed to run server");
    });

    Ok(local_addr)
}

/// Initialize and start the backend server
pub async fn start_server(
    config: Config,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    init_tracing(&config);
    tracing::info!("Starting ziee backend server");
    init_data_dir(&config);

    let setup = setup_server(config, vec![]).await?;
    run_server(setup.app, setup.addr).await
}

/// Initialize and start the backend server with custom routes and event handlers
/// Allows desktop/external apps to add custom endpoints and event handlers
pub async fn start_server_with_routes<F>(
    config: Config,
    route_builder: F,
    additional_handlers: Vec<Arc<dyn core::EventHandler>>,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(Router, Arc<JwtService>) -> Router,
{
    init_tracing(&config);
    tracing::info!("Starting ziee backend server (with custom routes)");
    init_data_dir(&config);

    let setup = setup_server(config, additional_handlers).await?;
    let app = route_builder(setup.app, setup.jwt_service);
    run_server(app, setup.addr).await
}

/// Find an available port in the given range
pub fn find_available_port(start: u16, end: u16) -> Option<u16> {
    (start..=end).find(|&port| std::net::TcpListener::bind(("127.0.0.1", port)).is_ok())
}

/// (Windows) True iff the LocalSystem code-sandbox helper service is reachable
/// (answers a `Ping`). Exposed so the integration-test harness — and, later,
/// desktop onboarding — can decide whether to trigger an elevated install
/// before the sandbox is exercised. See
/// `modules::code_sandbox::backend::helper_service`.
#[cfg(windows)]
pub fn sandbox_helper_is_running() -> bool {
    modules::code_sandbox::backend::helper_service::client::is_running()
}

/// Cleanup server resources
pub async fn cleanup_server() {
    tracing::info!("Cleaning up server resources...");
    core::database::cleanup_database().await;
}

/// Generate OpenAPI specification
pub async fn generate_openapi(
    output_dir: &str,
    config_file: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    openapi::generate_openapi_spec(output_dir, config_file).await?;
    Ok(())
}
