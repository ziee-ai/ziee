//! knowledge_base routes: the JSON-RPC MCP endpoint + the typed REST surface.

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};
use axum::routing::post;

use super::handlers;

pub fn knowledge_base_router() -> ApiRouter {
    ApiRouter::new()
        // JSON-RPC dispatch over a single path — plain `route`, not `api_route`.
        .route("/knowledge-base/mcp", post(handlers::jsonrpc_handler))
        // KB CRUD.
        .api_route(
            "/knowledge-bases",
            get_with(handlers::list_kbs, handlers::list_kbs_docs)
                .post_with(handlers::create_kb, handlers::create_kb_docs),
        )
        .api_route(
            "/knowledge-bases/{id}",
            get_with(handlers::get_kb, handlers::get_kb_docs)
                .put_with(handlers::update_kb, handlers::update_kb_docs)
                .delete_with(handlers::delete_kb, handlers::delete_kb_docs),
        )
        // Documents.
        .api_route(
            "/knowledge-bases/{id}/documents",
            get_with(handlers::list_documents, handlers::list_documents_docs)
                .post_with(handlers::attach_documents, handlers::attach_documents_docs),
        )
        .api_route(
            "/knowledge-bases/{id}/documents/{file_id}",
            delete_with(handlers::remove_document, handlers::remove_document_docs),
        )
        .api_route(
            "/knowledge-bases/{id}/documents/{file_id}/reindex",
            post_with(handlers::reindex_document, handlers::reindex_document_docs),
        )
        // Attach to conversation / project.
        .api_route(
            "/conversations/{cid}/knowledge-bases/{kb_id}",
            put_with(handlers::attach_conversation, handlers::attach_conversation_docs)
                .delete_with(handlers::detach_conversation, handlers::detach_conversation_docs),
        )
        .api_route(
            "/projects/{pid}/knowledge-bases/{kb_id}",
            put_with(handlers::attach_project, handlers::attach_project_docs)
                .delete_with(handlers::detach_project, handlers::detach_project_docs),
        )
}
