pub mod config;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod sandbox;
pub mod tools;
pub mod types;

pub use repository::CodeSandboxRepository;
pub use routes::code_sandbox_router;

use std::path::PathBuf;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use uuid::Uuid;

use crate::core::Repos;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};
use types::CodeSandboxState;

pub fn code_sandbox_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"code-sandbox.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static CODE_SANDBOX_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "code_sandbox",
    order: 70,
    description: "Code sandbox execution via bwrap",
    constructor: || Box::new(CodeSandboxModule::new()),
};

pub struct CodeSandboxModule {
    enabled: bool,
    server_url: Option<String>,
}

impl CodeSandboxModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            server_url: None,
        }
    }
}

impl AppModule for CodeSandboxModule {
    fn name(&self) -> &'static str {
        "code_sandbox"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "Code sandbox execution — runs LLM-generated code via bwrap with per-conversation workspaces"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn std::error::Error>> {
        let config = &ctx.config;

        let sandbox_cfg = match &config.code_sandbox {
            Some(c) if c.enabled => c,
            _ => {
                tracing::info!("code_sandbox: disabled or not configured, skipping");
                return Ok(());
            }
        };

        let data_dir = config
            .app
            .as_ref()
            .map(|a| PathBuf::from(&a.data_dir))
            .unwrap_or_else(|| PathBuf::from("./data"));

        let sandboxes_dir = data_dir.join("sandboxes");
        std::fs::create_dir_all(&sandboxes_dir)
            .map_err(|e| format!("Failed to create sandboxes directory: {}", e))?;

        let rootfs_path = PathBuf::from(&sandbox_cfg.rootfs_path);

        let host = &config.server.host;
        let port = config.server.port;
        let url = format!("http://{}:{}/api/code-sandbox", host, port);
        let api_base_url = format!("http://{}:{}/api", host, port);

        config::init_sandbox_config(CodeSandboxState {
            data_dir,
            rootfs_path,
            base_url: api_base_url,
            jwt_secret: config.jwt.secret.clone(),
        });
        self.enabled = true;
        self.server_url = Some(url.clone());

        let server_id = code_sandbox_server_id();
        tokio::spawn(async move {
            if let Err(e) = Repos.code_sandbox.upsert_builtin_server(server_id, &url).await {
                tracing::error!("code_sandbox: failed to upsert built-in MCP server: {}", e);
            } else {
                tracing::info!("code_sandbox: built-in MCP server registered (id={})", server_id);
            }
            if let Err(e) = Repos.code_sandbox.upsert_builtin_user_defaults(server_id).await {
                tracing::error!("code_sandbox: failed to upsert user defaults: {}", e);
            } else {
                tracing::info!("code_sandbox: user defaults updated for server (id={})", server_id);
            }
        });

        tracing::info!("code_sandbox: initialized, sandboxes at {:?}", sandboxes_dir);
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if !self.enabled { return router; }
        router.merge(code_sandbox_router())
    }
}

impl Default for CodeSandboxModule {
    fn default() -> Self {
        Self::new()
    }
}
