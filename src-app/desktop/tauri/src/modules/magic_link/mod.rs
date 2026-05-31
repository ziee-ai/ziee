//! Magic-link login tokens.
//!
//! Issued by the desktop admin (admin-only + localhost-gated through
//! the remote_access middleware); consumed by a phone hitting the QR
//! URL on the public tunnel (unauthenticated, rate-limited).
//!
//! Tokens are 32-byte random values. The plaintext is returned ONCE
//! on issue and the SHA-256 hash is stored as the row primary key —
//! a DB dump leaks nothing useful. Tokens are single-use + 5-min TTL.

pub mod handlers;
pub mod models;
pub mod repository;
pub mod routes;

pub use routes::magic_link_routes;

use anyhow::Result;
use tauri::App;
use ziee::ApiRouter;

use crate::module_api::DesktopModule;

pub struct MagicLinkModule;

impl MagicLinkModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MagicLinkModule {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopModule for MagicLinkModule {
    fn name(&self) -> &'static str {
        "magic_link"
    }

    fn description(&self) -> &'static str {
        "Magic-link issue + exchange for the desktop tunnel"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        // Spawn the magic-link reaper. Without this, used + expired
        // rows accumulate forever — each rotation of the on-page QR
        // appends a row, and the admin opening the page for an hour
        // (15/hour rotation) puts ~360/day on the table.
        //
        // 60-minute tick is fine: tokens are 5-min TTL, so the cleanup
        // lag is at most an hour past expiry. We poll for repos
        // because init() fires before the embedded DB pool is ready.
        tauri::async_runtime::spawn(async move {
            if !crate::modules::remote_access::auto_start::wait_for_repos_ready(
                std::time::Duration::from_secs(60),
            )
            .await
            {
                tracing::warn!(
                    "magic_link.reaper: gave up waiting for Repos to be initialized; reaper not started"
                );
                return;
            }
            let repo = repository::MagicLinkRepository::new(ziee::Repos.pool().clone());
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
            tick.tick().await; // skip the immediate first tick
            tracing::info!("magic_link.reaper: started; tick=3600s");
            loop {
                tick.tick().await;
                match repo.reap_old().await {
                    Ok(n) if n > 0 => {
                        tracing::info!(deleted = n, "magic_link.reaper: tick");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "magic_link.reaper: tick failed");
                    }
                }
            }
        });
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::magic_link_routes())
    }
}
