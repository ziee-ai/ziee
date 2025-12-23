//! Auth Bootstrap
//!
//! Desktop user bootstrapping - ensures required users exist

use anyhow::Result;

/// Ensure desktop admin user exists (create on first run)
pub async fn ensure_desktop_admin() -> Result<()> {
    let has_admin = ziee_chat::Repos
        .user
        .has_admin()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to check admin: {}", e))?;

    if !has_admin {
        tracing::info!("No admin exists, creating desktop admin user");

        let password_hash = ziee_chat::hash_password("desktop-auto-login")
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

        ziee_chat::Repos
            .app
            .create_admin_user("admin", "admin@localhost", &password_hash, None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create admin: {}", e))?;

        tracing::info!("Desktop admin user created successfully");
    }

    Ok(())
}
