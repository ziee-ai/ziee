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
}

static ELICITATION_REGISTRY: Lazy<Mutex<HashMap<Uuid, ElicitationEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Register a pending elicitation keyed by a per-elicitation random UUID.
pub fn register(elicitation_id: Uuid, tx: oneshot::Sender<ElicitationResponse>, content_id: Option<Uuid>) {
    let entry = ElicitationEntry { tx, content_id };
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
