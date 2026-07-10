//! REST surface for the notification inbox (all owner-scoped).

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with},
};

use super::handlers;

pub fn notification_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/notifications",
            get_with(handlers::list_notifications, handlers::list_notifications_docs),
        )
        .api_route(
            "/notifications/unread-count",
            get_with(handlers::unread_count, handlers::unread_count_docs),
        )
        .api_route(
            "/notifications/read-all",
            post_with(handlers::mark_all_read, handlers::mark_all_read_docs),
        )
        .api_route(
            "/notifications/{id}",
            get_with(handlers::get_notification, handlers::get_notification_docs)
                .delete_with(handlers::delete_notification, handlers::delete_notification_docs),
        )
        .api_route(
            "/notifications/{id}/read",
            post_with(handlers::mark_read, handlers::mark_read_docs),
        )
}
