// Elicitation channel registry
//
// Maps elicitation_id (random Uuid) → ElicitationEntry.
// A fresh UUID is generated each time an elicitation/create event fires in http.rs.
// Using a per-elicitation random UUID (not message_id) ensures sequential elicitations
// within the same tool call each get their own unique key.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::models::ElicitationResponse;

struct ElicitationEntry {
    tx: oneshot::Sender<ElicitationResponse>,
    /// UUID of the message_contents row for this elicitation (None if message_id was absent)
    content_id: Option<Uuid>,
    /// Owner user_id. Bound by the chat extension layer (which knows
    /// the calling user) once the ElicitationStartedNotification is
    /// observed. None means "not yet bound" — the respond handler
    /// MUST reject in that case (fail-closed) to defend against the
    /// race where the elicitation is created but the binding hook
    /// hasn't fired. Closes 02-permissions F-04.
    owner_user_id: Option<Uuid>,
}

static ELICITATION_REGISTRY: Lazy<Mutex<HashMap<Uuid, ElicitationEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Register a pending elicitation keyed by a per-elicitation random UUID.
/// owner_user_id is None at first; the chat extension calls
/// `bind_owner` after the notification fires so the respond handler
/// can verify the responder. See 02-permissions F-04.
pub fn register(elicitation_id: Uuid, tx: oneshot::Sender<ElicitationResponse>, content_id: Option<Uuid>) {
    let entry = ElicitationEntry { tx, content_id, owner_user_id: None };
    match ELICITATION_REGISTRY.lock() {
        Ok(mut map) => {
            map.insert(elicitation_id, entry);
        }
        Err(poisoned) => {
            tracing::error!("[elicitation] registry Mutex poisoned — recovering");
            poisoned.into_inner().insert(elicitation_id, entry);
        }
    }
}

/// Bind the owning user_id to a registered elicitation. Called by the
/// chat extension layer (which knows the calling user_id from
/// `context.user_id`) once it consumes the
/// ElicitationStartedNotification. Idempotent; no-op if the entry is
/// already gone (responded / cancelled).
pub fn bind_owner(elicitation_id: Uuid, user_id: Uuid) {
    let mut map = match ELICITATION_REGISTRY.lock() {
        Ok(m) => m,
        Err(poisoned) => {
            tracing::error!("[elicitation] registry Mutex poisoned — recovering");
            poisoned.into_inner()
        }
    };
    if let Some(entry) = map.get_mut(&elicitation_id) {
        entry.owner_user_id = Some(user_id);
    }
}

/// Verify the elicitation exists AND belongs to the supplied user.
/// Returns:
///   - None if the entry doesn't exist (404 to caller)
///   - Some(false) if it exists but the owner hasn't been bound yet, or
///     the user_id doesn't match (403 to caller — fail-closed)
///   - Some(true) on match (proceed)
pub fn owner_matches(elicitation_id: Uuid, user_id: Uuid) -> Option<bool> {
    let map = match ELICITATION_REGISTRY.lock() {
        Ok(m) => m,
        Err(poisoned) => {
            tracing::error!("[elicitation] registry Mutex poisoned — recovering");
            poisoned.into_inner()
        }
    };
    map.get(&elicitation_id)
        .map(|e| e.owner_user_id == Some(user_id))
}

/// Deliver the user's response.
/// Returns `(found, content_id)`: found=true if registry had the entry, content_id is the DB row id.
pub fn respond(elicitation_id: Uuid, response: ElicitationResponse) -> (bool, Option<Uuid>) {
    let entry = match ELICITATION_REGISTRY.lock() {
        Ok(mut map) => map.remove(&elicitation_id),
        Err(poisoned) => {
            tracing::error!("[elicitation] registry Mutex poisoned — recovering");
            poisoned.into_inner().remove(&elicitation_id)
        }
    };

    match entry {
        Some(e) => {
            // Ignore send errors — the SSE loop may have already closed
            let _ = e.tx.send(response);
            (true, e.content_id)
        }
        None => (false, None),
    }
}

/// Remove a pending elicitation (called when SSE channel closes).
/// Returns the content_id of the DB row if one was created.
pub fn remove(elicitation_id: Uuid) -> Option<Uuid> {
    let entry = match ELICITATION_REGISTRY.lock() {
        Ok(mut map) => map.remove(&elicitation_id),
        Err(poisoned) => {
            tracing::error!("[elicitation] registry Mutex poisoned — recovering");
            poisoned.into_inner().remove(&elicitation_id)
        }
    };
    entry.map(|e| e.content_id).flatten()
}
