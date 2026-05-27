use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, put_with},
};

use super::handlers::*;

pub fn assistant_core_memory_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/assistants/{assistant_id}/core-memory",
            get_with(list_blocks, list_blocks_docs),
        )
        .api_route(
            "/assistants/core-memory",
            put_with(upsert_block, upsert_block_docs),
        )
        .api_route(
            "/assistants/{assistant_id}/core-memory/{block_label}",
            delete_with(delete_block, delete_block_docs),
        )
}
