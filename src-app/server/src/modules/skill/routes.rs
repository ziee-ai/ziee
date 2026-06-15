//! Skill REST routes. User vs admin split mirrors `mcp/routes.rs`.

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::dev_handlers;
use super::handlers::*;

pub fn user_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route("/skills", get_with(list_user_skills, list_user_skills_docs))
        .api_route(
            "/skills/available",
            get_with(list_available_skills, list_available_skills_docs),
        )
        .api_route(
            "/skills/install-from-hub",
            // Thin re-bind of `hub::handlers::create_skill_from_hub` at the
            // canonical user-facing path. Single implementation; the hub
            // module owns the install pipeline.
            post_with(install_from_hub, install_from_hub_docs),
        )
        // B6 dev/local import + validate.
        .api_route(
            "/skills/import",
            post_with(dev_handlers::import_skill, dev_handlers::import_skill_docs),
        )
        .api_route(
            "/skills/validate",
            post_with(dev_handlers::validate_skill, dev_handlers::validate_skill_docs),
        )
        .api_route("/skills/{id}", get_with(get_user_skill, get_user_skill_docs))
        .api_route(
            "/skills/{id}",
            put_with(update_user_skill, update_user_skill_docs),
        )
        .api_route(
            "/skills/{id}",
            delete_with(delete_user_skill, delete_user_skill_docs),
        )
        .api_route(
            "/skills/{id}/hide-in-conversation",
            post_with(hide_skill_in_conversation, hide_skill_in_conversation_docs),
        )
        .api_route(
            "/skills/{id}/hide-in-conversation/{conversation_id}",
            delete_with(
                unhide_skill_in_conversation,
                unhide_skill_in_conversation_docs,
            ),
        )
}

pub fn admin_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/skills/system",
            get_with(list_system_skills, list_system_skills_docs),
        )
        .api_route(
            "/skills/system/install-from-hub",
            post_with(install_system_from_hub, install_system_from_hub_docs),
        )
        .api_route(
            "/skills/system/{id}",
            get_with(get_system_skill, get_system_skill_docs),
        )
        .api_route(
            "/skills/system/{id}",
            put_with(update_system_skill, update_system_skill_docs),
        )
        .api_route(
            "/skills/system/{id}",
            delete_with(delete_system_skill, delete_system_skill_docs),
        )
        .api_route(
            "/skills/system/{id}/groups",
            get_with(get_skill_groups, get_skill_groups_docs),
        )
        .api_route(
            "/skills/system/{id}/groups",
            post_with(set_skill_groups, set_skill_groups_docs),
        )
        .api_route(
            "/skills/system/{id}/groups/{group_id}",
            delete_with(remove_skill_from_group, remove_skill_from_group_docs),
        )
}
