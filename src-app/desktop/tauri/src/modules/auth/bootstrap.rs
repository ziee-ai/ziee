//! Auth Bootstrap
//!
//! Desktop user bootstrapping - ensures required users exist

use anyhow::Result;
use sqlx::PgPool;

/// Ensure desktop admin user exists (create on first run).
///
/// Chunk BG-3: the pool is threaded (from the `ServerBoot` `BootHandle`) rather
/// than reaching the global `ziee::Repos`. `UserRepository::new(pool)` (owner
/// read) + `AppRepository::new(pool)` (the app-side owner-create domain CRUD,
/// kept app-side by BA) are the same repositories `Repos.{user,app}` build from
/// the same pool, so this is behaviour-identical while de-globalizing the
/// desktop consumer surface.
pub async fn ensure_desktop_admin(pool: &PgPool) -> Result<()> {
    let has_admin = ziee::UserRepository::new(pool.clone())
        .has_admin()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to check admin: {}", e))?;

    if !has_admin {
        tracing::info!("No admin exists, creating desktop admin user");

        let password_hash = ziee::hash_password("desktop-auto-login")
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

        ziee::AppRepository::new(pool.clone())
            .create_admin_user("admin", "admin@localhost", &password_hash, None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create admin: {}", e))?;

        tracing::info!("Desktop admin user created successfully");
    }

    Ok(())
}
