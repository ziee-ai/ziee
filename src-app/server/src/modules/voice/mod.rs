//! Voice dictation: a managed whisper.cpp speech-to-text runtime.
//!
//! Fully LOCAL, privacy-preserving voice INPUT for the chat composer. A managed
//! `whisper-server` instance (downloaded on demand from the `ziee-ai/whisper.cpp`
//! fork, like the LLM engines) transcribes browser-recorded audio on-device; the
//! transcript is inserted into the composer for the user to review before
//! sending (never auto-sent).
//!
//! Mirrors `llm_local_runtime` (version registry + download + update + admin UI +
//! settings + health/idle-reap lifecycle), scoped to a SINGLE hot-swappable
//! instance. Fail-soft like pgvector/biomcp: when whisper is unavailable the mic
//! self-disables and the app still works.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod binary_manager;
pub mod handlers;
pub mod model;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;

pub use repository::VoiceRepository;

#[distributed_slice(MODULE_ENTRIES)]
static VOICE_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "voice",
    // Near the runtime modules (llm_local_runtime=32, llm_model=35). 36 is free.
    order: 36,
    description: "Managed whisper.cpp speech-to-text runtime for chat composer voice dictation",
    constructor: || Box::new(VoiceModule::new()),
};

pub struct VoiceModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl VoiceModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for VoiceModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for VoiceModule {
    fn name(&self) -> &'static str {
        "voice"
    }

    fn description(&self) -> &'static str {
        "Managed whisper.cpp speech-to-text runtime for voice dictation"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Deploy-level kill switch — ON by default (an absent `voice:` config
        // section means enabled). Operators opt OUT with `voice: { enabled:
        // false }`; an admin cannot re-enable it (distinct from the runtime
        // `voice_runtime_settings.enabled` toggle).
        let enabled = ctx.config.voice.as_ref().map(|c| c.enabled).unwrap_or(true);
        if !enabled {
            tracing::info!("voice: disabled in config; skipping registration");
            return Ok(());
        }

        // The reaper (idle-unload + health monitor) is spawned once the
        // deployment/reaper layer is wired in; nothing to spawn yet.
        tracing::info!("voice: enabled (whisper runtime lazily started on first transcribe)");
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::voice_router())
    }
}
