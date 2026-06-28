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
    entry.and_then(|e| e.content_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::mcp::elicitation::models::ElicitationResponse;

    fn decline() -> ElicitationResponse {
        ElicitationResponse { action: "decline".into(), content: None }
    }

    /// The owner-binding contract the chat-extension notification handler
    /// (mcp.rs `execute_approved_tools_sync`) relies on for F-04:
    ///   - before `bind_owner`, ownership is fail-closed (`Some(false)`),
    ///   - after binding, only the bound user matches,
    ///   - an unknown elicitation is `None` (→ 404, not 403).
    #[test]
    fn owner_binding_is_fail_closed_until_bound() {
        let id = Uuid::new_v4();
        let owner = Uuid::new_v4();
        let stranger = Uuid::new_v4();
        let (tx, _rx) = oneshot::channel::<ElicitationResponse>();

        // Unknown elicitation → None.
        assert_eq!(owner_matches(id, owner), None);

        register(id, tx, Some(Uuid::new_v4()));
        // Registered but unbound → fail-closed (Some(false)) for everyone.
        assert_eq!(owner_matches(id, owner), Some(false));

        bind_owner(id, owner);
        assert_eq!(owner_matches(id, owner), Some(true), "bound owner matches");
        assert_eq!(
            owner_matches(id, stranger),
            Some(false),
            "a different user never matches"
        );

        // cleanup
        let _ = respond(id, decline());
    }

    /// `respond` delivers the answer to the waiting receiver exactly once and
    /// removes the entry; a second respond / owner check no longer finds it.
    #[test]
    fn respond_delivers_once_and_removes_entry() {
        let id = Uuid::new_v4();
        let content_id = Uuid::new_v4();
        let (tx, rx) = oneshot::channel::<ElicitationResponse>();
        register(id, tx, Some(content_id));
        bind_owner(id, Uuid::new_v4());

        let (found, cid) = respond(id, decline());
        assert!(found, "respond finds the registered entry");
        assert_eq!(cid, Some(content_id), "respond returns the content_id");
        assert_eq!(rx.blocking_recv().unwrap().action, "decline");

        // Entry is gone now.
        assert_eq!(owner_matches(id, Uuid::new_v4()), None);
        assert_eq!(respond(id, decline()).0, false, "second respond finds nothing");
    }

    /// `remove` (the cancellation path) drops the entry and returns its
    /// content_id so the caller can mark the DB row cancelled; binding a
    /// removed elicitation is a no-op.
    #[test]
    fn remove_cancels_and_returns_content_id() {
        let id = Uuid::new_v4();
        let content_id = Uuid::new_v4();
        let (tx, _rx) = oneshot::channel::<ElicitationResponse>();
        register(id, tx, Some(content_id));

        assert_eq!(remove(id), Some(content_id));
        assert_eq!(owner_matches(id, Uuid::new_v4()), None, "removed → gone");
        // bind_owner on a removed/unknown entry is a harmless no-op.
        bind_owner(id, Uuid::new_v4());
        assert_eq!(owner_matches(id, Uuid::new_v4()), None);
    }
}
