//! Tauri command tests (Layer 1).
//!
//! Call the underlying `#[tauri::command]` async functions directly,
//! using a `tauri::test::mock_builder` app to provide the `State` map
//! when a command needs it. No IPC dispatch / capabilities ACL — that
//! belongs in Layer 3 (WebDriver E2E).
//!
//! What's covered here:
//!   - `get_server_port` — depends on `State<BackendState>`; verify the
//!     command returns the port stashed on the app's state map.
//!   - `auto_login` — verify the not-ready early-return path when
//!     `JWT_SERVICE` is unset. The happy path is covered by
//!     `auth_tests.rs::mint_admin_login_*` (calls the extracted helper
//!     directly with a real `JwtService` + Repos pool).

use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::Manager;
use ziee_desktop::modules::auth::commands::auto_login;
use ziee_desktop::modules::backend::commands::get_server_port;
use ziee_desktop::modules::backend::BackendState;
use ziee_desktop::register_desktop_invoke_handler;

#[tokio::test]
async fn get_server_port_returns_the_managed_state_port() {
    let app = register_desktop_invoke_handler(mock_builder())
        .build(mock_context(noop_assets()))
        .expect("mock_builder build");

    app.manage(BackendState::new(8123));

    let state = app.state::<BackendState>();
    let port = get_server_port(state).await.expect("get_server_port");

    assert_eq!(port, 8123);
}

#[tokio::test]
async fn auto_login_returns_not_ready_error_when_jwt_service_unset() {
    // The JWT_SERVICE OnceLock inside `modules::backend` is NOT set in
    // this test process. `auto_login` should fail fast with the exact
    // error string the desktop-base retry loop matches on.
    let err = auto_login()
        .await
        .expect_err("auto_login should error when JWT_SERVICE is unset");

    assert!(
        err.contains("Server not ready"),
        "expected 'Server not ready' in error, got: {err}"
    );
}
