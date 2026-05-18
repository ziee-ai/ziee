use aide::axum::{ApiRouter, routing::{get_with, post_with}};

use super::handlers::{
    download_workspace_file, download_workspace_file_docs,
    handle_mcp_request, handle_mcp_request_docs,
};

pub fn code_sandbox_router() -> ApiRouter {
    ApiRouter::new()
        .api_route("/code-sandbox", post_with(handle_mcp_request, handle_mcp_request_docs))
        .api_route(
            "/code-sandbox/file/download",
            get_with(download_workspace_file, download_workspace_file_docs),
        )
}
