// LLM Repository routes configuration

use aide::axum::{routing::{get_with, post_with}, ApiRouter};

use super::handlers::*;

/// LLM Repository management routes
pub fn llm_repository_router() -> ApiRouter {
    ApiRouter::new()
        .api_route("/llm-repositories", get_with(list_repositories, list_repositories_docs))
        .api_route("/llm-repositories", post_with(create_repository, create_repository_docs))
        .api_route("/llm-repositories/test", post_with(test_repository_connection, test_repository_connection_docs))
        .api_route("/llm-repositories/{repository_id}", get_with(get_repository, get_repository_docs))
        .api_route("/llm-repositories/{repository_id}", post_with(update_repository, update_repository_docs))
        .api_route("/llm-repositories/{repository_id}", aide::axum::routing::delete_with(delete_repository, delete_repository_docs))
}
