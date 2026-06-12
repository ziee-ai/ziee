//! Realtime-sync emission for file/version changes.
//!
//! A file's bytes/metadata change only by appending a version (MCP edit,
//! sandbox version-back) or restoring one — all of which move the head. We
//! notify the owner's other devices (owner-scoped) with the stable file_id;
//! the client refetches the file + its versions. Notify-and-refetch only.

use uuid::Uuid;

use crate::modules::sync::event::{publish, Audience, SyncAction, SyncEntity};

/// Notify the owner that a file's head/version set changed. `origin_conn` is
/// the originating SSE connection (skipped for self-echo) when known, else
/// `None` (the originating device performs one redundant refetch — harmless).
pub fn publish_file_changed_with_origin(user_id: Uuid, file_id: Uuid, origin_conn: Option<Uuid>) {
    publish(
        SyncEntity::File,
        SyncAction::Update,
        file_id,
        Audience::owner(user_id),
        origin_conn,
    );
}

/// Convenience wrapper for background/tool-driven changes with no originating
/// SSE connection (files_mcp edits, sandbox version-back, restore).
pub fn publish_file_changed(user_id: Uuid, file_id: Uuid) {
    publish_file_changed_with_origin(user_id, file_id, None);
}
