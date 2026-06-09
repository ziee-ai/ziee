//! Tier 8 — TRULY-PUBLISHED MCP package smoke (rootfs-gated, network
//! egress required, opt-in via `--ignored`).
//!
//! Unlike the Tier 6 echo fixture (which is an inline python script we
//! wrote), this tier exercises a REAL published MCP package end-to-end:
//! the `mcp-server-fetch` package on PyPI. The package is installed
//! into the sandboxed per-server workspace at test setup time via
//! `python3 -m pip install --break-system-packages --user
//! mcp-server-fetch`, then exec'd via `python3 -m mcp_server_fetch`.
//!
//! Why pip-install vs `uvx mcp-server-fetch`: the embedded `uv` and
//! `bun` binaries on a Mac/Windows host are host-arch (Mach-O /
//! Windows PE) and can't execute inside the Linux sandbox rootfs. The
//! Linux native sandbox path bind-mounts the host's Linux-arch uv so
//! `uvx` works there; the VM paths don't have a Linux-arch uv
//! available in v1. pip is in the rootfs on every platform, so this
//! is the cross-platform route to a real published package.
//!
//! What this proves that Tier 6's echo doesn't:
//!   - The sandboxed `--share-net` actually reaches PyPI + a real
//!     fetch target (https://example.com)
//!   - Python package dependency resolution works inside the bwrap
//!     filesystem layout (--ro-bind /usr, writable /home/sandboxuser)
//!   - pip can install into the per-server workspace and python can
//!     import from there
//!   - A real-world MCP server's `initialize` + `tools/list` +
//!     `tools/call` flow round-trips through the sandbox
//!
//! Rootfs-gated + network-gated. `#[ignore]`'d by default. Heavy:
//! first run pip-installs ~10MB; subsequent runs use the cached
//! ~/.local/lib in the per-server workspace.

use serde_json::json;

use super::harness;
use crate::common::test_helpers;

/// Python wrapper: pip-install mcp-server-fetch into $HOME/.local
/// (idempotent — skip when already importable), then os.execvp into
/// `python3 -m mcp_server_fetch` so the server takes over the stdio
/// pipes from the wrapper. Using python3 as the command (which IS in
/// the stdio ALLOWED_COMMANDS list) instead of bash (which isn't).
const PIP_INSTALL_AND_RUN: &str = r#"
import os, sys, subprocess
os.environ["PYTHONUSERBASE"] = os.path.expanduser("~/.local")
os.environ["PATH"] = f"{os.environ['PYTHONUSERBASE']}/bin:{os.environ.get('PATH','')}"
try:
    import mcp_server_fetch  # already installed → skip
except ImportError:
    r = subprocess.run(
        [sys.executable, "-m", "pip", "install", "--user", "--quiet",
         "--break-system-packages", "mcp-server-fetch"],
        check=False,
    )
    if r.returncode != 0:
        sys.stderr.write("pip-install mcp-server-fetch FAILED — check sandbox network egress\n")
        sys.exit(1)
# Replace self with the real MCP server so stdin/stdout pipes pass
# straight through to it (rmcp on the host now talks to mcp_server_fetch).
os.execvp(sys.executable, [sys.executable, "-m", "mcp_server_fetch"])
"#;

async fn create_real_fetch_server(
    server: &crate::common::TestServer,
    admin_token: &str,
) -> String {
    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({
            "name": format!("tier8-fetch-{}", uuid::Uuid::new_v4()),
            "display_name": "Tier 8 mcp-server-fetch (real PyPI)",
            "enabled": true,
            "transport_type": "stdio",
            "command": "python3",
            "args": ["-c", PIP_INSTALL_AND_RUN],
            "environment_variables": {},
            "timeout_seconds": 180,
            "run_in_sandbox": true,
        }))
        .send()
        .await
        .expect("create system server (tier8 real-fetch)");
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(
        status, 201,
        "create system server for tier8 failed: {body}"
    );
    body["id"].as_str().expect("server id").to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn real_mcp_server_fetch_via_pip_install_inside_sandbox() {
    let Some(server) = harness::enabled_test_server().await else {
        eprintln!("tier8 skipped: sandbox not bootable on this host");
        return;
    };

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::create",
            "mcp_servers_admin::read",
            "mcp_servers::read",
        ],
    )
    .await;

    let server_id = create_real_fetch_server(&server, &admin.token).await;

    // GET tools — forces a sandboxed connect → pip-install → exec
    // mcp_server_fetch → list_tools. First run downloads ~10MB; the
    // 120s timeout in the server config should cover it.
    let tools_url = server.api_url(&format!("/mcp/servers/{server_id}/tools"));
    let tools_resp = reqwest::Client::new()
        .get(&tools_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .timeout(std::time::Duration::from_secs(180))
        .send()
        .await
        .expect("list tools http (tier8)");
    let tools_status = tools_resp.status();
    let tools_body: serde_json::Value = tools_resp.json().await.expect("tools json");

    assert_eq!(
        tools_status, 200,
        "tier8 list_tools failed (pip-install + server boot inside sandbox): {tools_body}"
    );
    let tools = tools_body["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        names.contains(&"fetch"),
        "mcp-server-fetch must expose 'fetch' tool. names={names:?}"
    );

    // Call fetch on example.com — REAL HTTP egress from inside the
    // sandbox. example.com's body famously contains "Example Domain".
    let call_url =
        server.api_url(&format!("/mcp/servers/{server_id}/tools/fetch/call"));
    let call_resp = reqwest::Client::new()
        .post(&call_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .timeout(std::time::Duration::from_secs(60))
        .json(&json!({
            "arguments": { "url": "https://example.com" }
        }))
        .send()
        .await
        .expect("fetch call http");
    let call_status = call_resp.status();
    let call_body: serde_json::Value = call_resp.json().await.expect("call json");
    assert_eq!(call_status, 200, "fetch call returned non-200: {call_body}");
    assert_eq!(call_body["is_error"], false, "fetch reported is_error: {call_body}");
    let serialized = serde_json::to_string(&call_body["content"]).unwrap_or_default();
    assert!(
        serialized.contains("Example Domain"),
        "real https://example.com fetch did not return expected content. \
         body={call_body}"
    );
}
