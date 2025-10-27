use super::models::*;
use super::service::UserGroupService;
use crate::common::{ApiResult, AppError, PaginationQuery};
use aide::axum::{
    routing::{delete_with, get_with, post_with, put_with},
    ApiRouter,
};
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct UserGroupState {
    pub service: Arc<UserGroupService>,
}

pub fn routes(state: UserGroupState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/user-groups",
            post_with(create_group, |op| {
                op.description("Create a new user group")
                    .id("UserGroup.create")
                    .tag("user-groups")
                    .response::<200, Json<UserGroup>>()
            }),
        )
        .api_route(
            "/user-groups",
            get_with(list_groups, |op| {
                op.description("List user groups with pagination")
                    .id("UserGroup.list")
                    .tag("user-groups")
                    .response::<200, Json<UserGroupListResponse>>()
            }),
        )
        .api_route(
            "/user-groups/{id}",
            get_with(get_group, |op| {
                op.description("Get user group by ID")
                    .id("UserGroup.get")
                    .tag("user-groups")
                    .response::<200, Json<UserGroup>>()
            }),
        )
        .api_route(
            "/user-groups/{id}",
            put_with(update_group, |op| {
                op.description("Update user group")
                    .id("UserGroup.update")
                    .tag("user-groups")
                    .response::<200, Json<UserGroup>>()
            }),
        )
        .api_route(
            "/user-groups/{id}",
            delete_with(delete_group, |op| {
                op.description("Delete user group")
                    .id("UserGroup.delete")
                    .tag("user-groups")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/user-groups/{id}/members",
            get_with(get_group_members, |op| {
                op.description("Get group members")
                    .id("UserGroup.getMembers")
                    .tag("user-groups")
                    .response::<200, Json<UserGroupMembersResponse>>()
            }),
        )
        .api_route(
            "/user-groups/{id}/members",
            post_with(assign_user_to_group, |op| {
                op.description("Assign user to group")
                    .id("UserGroup.assignUser")
                    .tag("user-groups")
                    .response::<200, Json<UserGroupMembership>>()
            }),
        )
        .api_route(
            "/user-groups/{id}/members/{user_id}",
            delete_with(remove_user_from_group, |op| {
                op.description("Remove user from group")
                    .id("UserGroup.removeUser")
                    .tag("user-groups")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/users/{user_id}/groups",
            get_with(get_user_groups, |op| {
                op.description("Get user's groups")
                    .id("UserGroup.getUserGroups")
                    .tag("user-groups")
                    .response::<200, Json<Vec<UserGroup>>>()
            }),
        )
        .with_state(state)
}

async fn create_group(
    State(state): State<UserGroupState>,
    Json(request): Json<CreateUserGroupRequest>,
) -> ApiResult<Json<UserGroup>> {
    let group = state.service.create_group(request).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(group)))
}

async fn get_group(
    State(state): State<UserGroupState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<UserGroup>> {
    let group = state
        .service
        .get_group(id)
        .await
        .map_err(|e| AppError::from(e).to_api_error())?
        .ok_or_else(|| AppError::not_found("Group").to_api_error())?;
    Ok((StatusCode::OK, Json(group)))
}

async fn list_groups(
    State(state): State<UserGroupState>,
    Query(pagination): Query<PaginationQuery>,
) -> ApiResult<Json<UserGroupListResponse>> {
    let response = state
        .service
        .list_groups(pagination.page, pagination.per_page)
        .await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(response)))
}

async fn update_group(
    State(state): State<UserGroupState>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateUserGroupRequest>,
) -> ApiResult<Json<UserGroup>> {
    let group = state
        .service
        .update_group(id, request)
        .await
        .map_err(|e| AppError::from(e).to_api_error())?
        .ok_or_else(|| AppError::not_found("Group").to_api_error())?;
    Ok((StatusCode::OK, Json(group)))
}

async fn delete_group(
    State(state): State<UserGroupState>,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    state.service.delete_group(id).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::NO_CONTENT, ()))
}

async fn assign_user_to_group(
    State(state): State<UserGroupState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AssignUserToGroupRequest>,
) -> ApiResult<Json<UserGroupMembership>> {
    let membership = state.service.assign_user(id, request.user_id, None).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(membership)))
}

async fn remove_user_from_group(
    State(state): State<UserGroupState>,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<()> {
    state.service.remove_user(id, user_id).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::NO_CONTENT, ()))
}

async fn get_group_members(
    State(state): State<UserGroupState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<UserGroupMembersResponse>> {
    let response = state.service.get_group_members(id).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(response)))
}

async fn get_user_groups(
    State(state): State<UserGroupState>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<Vec<UserGroup>>> {
    let groups = state.service.get_user_groups(user_id).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(groups)))
}
