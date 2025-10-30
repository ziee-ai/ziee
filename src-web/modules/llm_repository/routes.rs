// LLM Repository routes configuration

use aide::axum::{routing::{get_with, post_with}, ApiRouter};
use axum::Json;
use sqlx::PgPool;

use crate::modules::permissions::with_permission;

use super::{
    handlers::*,
    models::LlmRepository,
    types::{
        LlmRepositoryListResponse,
        TestRepositoryConnectionResponse,
    },
    permissions::*,
};

/// LLM Repository management routes
pub fn llm_repository_router() -> ApiRouter<PgPool> {
    ApiRouter::new()
        .api_route("/llm-repositories", get_with(list_repositories, list_repositories_docs))
        .api_route("/llm-repositories", post_with(create_repository, create_repository_docs))
        .api_route("/llm-repositories/test", post_with(test_repository_connection, test_repository_connection_docs))
        .api_route("/llm-repositories/{repository_id}", get_with(get_repository, get_repository_docs))
        .api_route("/llm-repositories/{repository_id}", post_with(update_repository, update_repository_docs))
        .api_route("/llm-repositories/{repository_id}", aide::axum::routing::delete_with(delete_repository, delete_repository_docs))
}

// =====================================================
// Documentation Functions
// =====================================================

fn list_repositories_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.list")
        .tag("LLM Repositories")
        .summary("List all LLM repositories with pagination")
        .response::<200, Json<LlmRepositoryListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

fn get_repository_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.get")
        .tag("LLM Repositories")
        .summary("Get LLM repository by ID")
        .response::<200, Json<LlmRepository>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

fn create_repository_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmRepositoriesCreate,)>(op)
        .id("LlmRepository.create")
        .tag("LLM Repositories")
        .summary("Create a new LLM repository")
        .response::<201, Json<LlmRepository>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

fn update_repository_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmRepositoriesEdit,)>(op)
        .id("LlmRepository.update")
        .tag("LLM Repositories")
        .summary("Update an existing LLM repository")
        .response::<200, Json<LlmRepository>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

fn delete_repository_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmRepositoriesDelete,)>(op)
        .id("LlmRepository.delete")
        .tag("LLM Repositories")
        .summary("Delete an LLM repository")
        .response::<204, ()>()
        .response_with::<400, (), _>(|res| res.description("Cannot delete built-in repository"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

fn test_repository_connection_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.test")
        .tag("LLM Repositories")
        .summary("Test repository connection without saving")
        .response::<200, Json<TestRepositoryConnectionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
