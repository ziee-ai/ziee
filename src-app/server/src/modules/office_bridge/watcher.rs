//! Live open/close document sync (ITEM-11).
//!
//! A background loop polls [`OfficePlatform::list_open_documents`] on a fixed
//! interval, diffs each snapshot against the previous one (keyed by the stable
//! `OpenDoc::full_name`), and emits an owner-scoped [`SyncEntity::OfficeDocument`]
//! notification on every open (`Create`) / close (`Delete`) so the frontend
//! "Open Office documents" panel refreshes live without a manual reload.
//!
//! Notify-and-refetch only (like the rest of `sync`): the event carries only
//! `{entity, action, id}` — never the document contents. The recipient
//! refetches its own permission-checked open-documents view.
//!
//! ## Audience — owner-scope (DEC-7)
//! Open Office documents are per-user desktop state, so every emit uses
//! `Audience::owner(user_id)`, never `everyone()`. On the single-user
//! ziee-desktop server the target user is resolved once in
//! `office_bridge::init()` (the oldest active admin, else the oldest active
//! user — the interactive desktop user) and passed into
//! [`watch_open_documents`]. The watcher never resolves it itself, so the
//! audience choice is explicit and testable via [`office_document_emit`].

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use uuid::Uuid;

use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};

use super::platform::{OfficePlatform, OpenDoc};

/// How often the watch loop re-enumerates open documents. Chosen in the
/// 3-5s band the plan calls for: responsive enough that the panel feels live,
/// cheap enough that the COM enumeration doesn't churn.
const POLL_INTERVAL: Duration = Duration::from_secs(4);

/// The open/close delta between two successive `list_open_documents` snapshots,
/// keyed by the stable `OpenDoc::full_name` identity.
#[derive(Debug, Clone, Default)]
pub struct OpenCloseDelta {
    /// Documents present in `now` but not in `prev` (newly opened).
    pub opened: Vec<OpenDoc>,
    /// Documents present in `prev` but not in `now` (newly closed).
    pub closed: Vec<OpenDoc>,
}

impl OpenCloseDelta {
    /// True when nothing opened or closed between the two snapshots.
    pub fn is_empty(&self) -> bool {
        self.opened.is_empty() && self.closed.is_empty()
    }
}

/// The `attach_method` stamped on a presence-only fallback entry (enumerated
/// from a window title when COM attach failed). Such an entry's `full_name` is a
/// window title, NOT the app's COM full path, so it does not share identity with
/// the same document's COM entry.
const TITLE_ONLY_ATTACH: &str = "window_enum_presence";

/// True when a doc was attached via a real COM identity (its `full_name` is the
/// app-qualified full path). A title-only presence fallback is excluded — see
/// [`diff_open_docs`].
fn has_com_identity(d: &OpenDoc) -> bool {
    d.attach_method != TITLE_ONLY_ATTACH
}

/// Diff two open-document snapshots by `full_name` (the app-qualified stable
/// identity). A document appearing → `opened`; disappearing → `closed`; present
/// in both (identity unchanged) → neither. Pure + allocation-only, so it's
/// directly unit-testable without a live platform or DB.
///
/// Only docs with a **real COM identity** participate in the open/close emission:
/// a `window_enum_presence` (title-only) fallback entry keys on a window title,
/// not the COM full path, so a transient COM-attach failure on one poll would
/// otherwise flip an already-known doc's identity and emit a spurious close+open
/// pair. Excluding the title-only entries from the diff keeps identity stable
/// across a flaky poll (the fallback is diagnostic-only; the panel's own refetch
/// still surfaces it in the list).
pub fn diff_open_docs(prev: &[OpenDoc], now: &[OpenDoc]) -> OpenCloseDelta {
    let prev_by_name: HashMap<&str, &OpenDoc> = prev
        .iter()
        .filter(|d| has_com_identity(d))
        .map(|d| (d.full_name.as_str(), d))
        .collect();
    let now_by_name: HashMap<&str, &OpenDoc> = now
        .iter()
        .filter(|d| has_com_identity(d))
        .map(|d| (d.full_name.as_str(), d))
        .collect();

    let opened = now
        .iter()
        .filter(|d| has_com_identity(d))
        .filter(|d| !prev_by_name.contains_key(d.full_name.as_str()))
        .cloned()
        .collect();
    let closed = prev
        .iter()
        .filter(|d| has_com_identity(d))
        .filter(|d| !now_by_name.contains_key(d.full_name.as_str()))
        .cloned()
        .collect();

    OpenCloseDelta { opened, closed }
}

/// Derive the stable sync entity id for a document from its `full_name`. Uses
/// `Uuid::new_v5` (SHA-1 over the URL namespace) so open + close of the same
/// document address the SAME `SyncEntity::OfficeDocument` id across snapshots
/// and process restarts — the client can key its refetch/removal off it.
pub fn office_document_id(full_name: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, full_name.as_bytes())
}

/// Construct the full emit tuple for one open/close transition WITHOUT
/// publishing. Returned as `(entity, action, id, audience)` so a unit test can
/// assert the owner-scope audience + stable id choice without a live registry
/// or DB (the whole point of TEST-14's emit assertion). The watch loop calls
/// this and hands the tuple straight to `sync_publish`.
pub fn office_document_emit(
    user_id: Uuid,
    action: SyncAction,
    full_name: &str,
) -> (SyncEntity, SyncAction, Uuid, Audience) {
    (
        SyncEntity::OfficeDocument,
        action,
        office_document_id(full_name),
        Audience::owner(user_id),
    )
}

/// Publish one open/close transition to the owning user's connections.
fn emit(user_id: Uuid, action: SyncAction, full_name: &str) {
    let (entity, action, id, audience) = office_document_emit(user_id, action, full_name);
    // Background/detached task ⇒ no originating SSE connection to suppress.
    sync_publish(entity, action, id, audience, None);
}

/// Background watch loop: poll `list_open_documents` on [`POLL_INTERVAL`], diff
/// against the previous snapshot, and emit an owner-scoped
/// `SyncEntity::OfficeDocument` `Create`/`Delete` per opened/closed document.
///
/// Resilience: a transient enumeration error (COM hiccup, Office mid-launch)
/// is logged and the PREVIOUS snapshot is retained — the loop keeps running so
/// a single flaky poll can't kill live sync or spuriously report every doc as
/// closed. `shutdown` lets a caller stop the loop cooperatively; production
/// spawns it fire-and-forget with a never-resolving future (process-lifetime),
/// mirroring the local-runtime reaper.
pub async fn watch_open_documents(
    platform: &dyn OfficePlatform,
    user_id: Uuid,
    shutdown: impl Future<Output = ()>,
) {
    tracing::info!(
        "office_bridge: open/close watch loop started (user={user_id}, tick {}s)",
        POLL_INTERVAL.as_secs()
    );

    // Seed the baseline from the first successful poll WITHOUT emitting — the
    // frontend loads the initial list via its own refetch, so we only want to
    // emit on subsequent *changes*, not replay the whole set at startup.
    let mut prev: Vec<OpenDoc> = match platform.list_open_documents().await {
        Ok(docs) => docs,
        Err(e) => {
            tracing::warn!("office_bridge: initial open-documents enumeration failed: {e}");
            Vec::new()
        }
    };

    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    // Skip the immediate first tick (interval fires once at t=0).
    ticker.tick().await;

    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                tracing::info!("office_bridge: open/close watch loop shutting down");
                return;
            }
            _ = ticker.tick() => {
                let now = match platform.list_open_documents().await {
                    Ok(docs) => docs,
                    Err(e) => {
                        // Keep `prev` so the next good poll diffs against the
                        // last known-good set (not an empty one, which would
                        // report every open doc as closed).
                        tracing::warn!(
                            "office_bridge: open-documents enumeration failed; \
                             keeping previous snapshot: {e}"
                        );
                        continue;
                    }
                };

                let delta = diff_open_docs(&prev, &now);
                if !delta.is_empty() {
                    for d in &delta.opened {
                        emit(user_id, SyncAction::Create, &d.full_name);
                    }
                    for d in &delta.closed {
                        emit(user_id, SyncAction::Delete, &d.full_name);
                    }
                    tracing::debug!(
                        "office_bridge: open/close delta (opened={}, closed={})",
                        delta.opened.len(),
                        delta.closed.len()
                    );
                }
                prev = now;
            }
        }
    }
}

// ─────────────────────────────────── Tests ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::office_bridge::platform::OfficeApp;

    fn doc(full_name: &str, app: OfficeApp) -> OpenDoc {
        OpenDoc {
            app,
            name: full_name.rsplit('\\').next().unwrap_or(full_name).to_string(),
            full_name: full_name.to_string(),
            path: None,
            saved: true,
            active: false,
            attach_method: "test".to_string(),
        }
    }

    /// TEST-14 (a) — a document appearing in the newer snapshot is `opened`;
    /// one disappearing is `closed`; one present in both is neither. Identity
    /// is keyed by `full_name`.
    #[test]
    fn test14_diff_computes_open_close_across_successive_snapshots() {
        let a = doc(r"C:\U\A.docx", OfficeApp::Word);
        let b = doc(r"C:\U\B.xlsx", OfficeApp::Excel);
        let c = doc(r"C:\U\C.pptx", OfficeApp::PowerPoint);

        // prev = {A, B}; now = {B, C}  ⇒  opened {C}, closed {A}, B unchanged.
        let prev = vec![a.clone(), b.clone()];
        let now = vec![b.clone(), c.clone()];
        let delta = diff_open_docs(&prev, &now);

        assert_eq!(delta.opened.len(), 1, "exactly one doc opened");
        assert_eq!(delta.opened[0].full_name, c.full_name);
        assert_eq!(delta.closed.len(), 1, "exactly one doc closed");
        assert_eq!(delta.closed[0].full_name, a.full_name);
        // B is in both snapshots ⇒ appears in neither set.
        assert!(!delta.opened.iter().any(|d| d.full_name == b.full_name));
        assert!(!delta.closed.iter().any(|d| d.full_name == b.full_name));
    }

    /// TEST-14 (b) — identical snapshots produce an empty delta (no spurious
    /// open/close churn while the set is stable).
    #[test]
    fn test14_unchanged_snapshot_yields_empty_delta() {
        let a = doc(r"C:\U\A.docx", OfficeApp::Word);
        let b = doc(r"C:\U\B.xlsx", OfficeApp::Excel);
        let snap = vec![a, b];
        let delta = diff_open_docs(&snap, &snap.clone());
        assert!(delta.is_empty(), "stable set ⇒ no opened/closed: {delta:?}");
    }

    /// TEST-14 (b cont.) — first-open from empty, and close-to-empty.
    #[test]
    fn test14_open_from_empty_and_close_to_empty() {
        let a = doc(r"C:\U\A.docx", OfficeApp::Word);

        let opened = diff_open_docs(&[], std::slice::from_ref(&a));
        assert_eq!(opened.opened.len(), 1);
        assert!(opened.closed.is_empty());

        let closed = diff_open_docs(std::slice::from_ref(&a), &[]);
        assert!(closed.opened.is_empty());
        assert_eq!(closed.closed.len(), 1);
    }

    /// TEST-14 (b cont.) — a title-only `window_enum_presence` fallback NEVER
    /// participates in the open/close emission, so it can't create a ghost,
    /// title-keyed `OfficeDocument` entity that flips against the same doc's real
    /// COM identity (the documented spurious close+open). Only docs with a real
    /// COM identity ever drive opened/closed.
    #[test]
    fn test14_title_only_fallback_never_emits() {
        let a_com = doc(r"C:\U\A.docx", OfficeApp::Word);
        let mut b_title = doc("B - Excel", OfficeApp::Excel);
        b_title.attach_method = "window_enum_presence".to_string();

        // A title-only entry appearing (from empty) emits nothing — no ghost open.
        let appeared = diff_open_docs(&[], std::slice::from_ref(&b_title));
        assert!(appeared.is_empty(), "a title-only entry must not open: {appeared:?}");

        // A title-only entry disappearing emits nothing — no ghost close.
        let vanished = diff_open_docs(std::slice::from_ref(&b_title), &[]);
        assert!(vanished.is_empty(), "a title-only entry must not close: {vanished:?}");

        // A COM doc alongside a title-only entry: only the COM doc is authoritative.
        // Adding the title-only entry to `now` must NOT register as an open, and
        // the stable COM doc registers as neither.
        let prev = vec![a_com.clone()];
        let now = vec![a_com.clone(), b_title.clone()];
        let delta = diff_open_docs(&prev, &now);
        assert!(
            delta.is_empty(),
            "a new title-only entry beside a stable COM doc must emit nothing: {delta:?}"
        );

        // The COM doc still drives real open/close when it genuinely comes/goes.
        let opened = diff_open_docs(std::slice::from_ref(&b_title), std::slice::from_ref(&a_com));
        assert_eq!(opened.opened.len(), 1, "the COM doc opening is still emitted");
        assert_eq!(opened.opened[0].full_name, a_com.full_name);
        assert!(opened.closed.is_empty(), "the title-only entry leaving is not a close");
    }

    /// TEST-14 (c) — the emit path constructs `SyncEntity::OfficeDocument` with
    /// an OWNER audience (never everyone) and a stable, full_name-derived id.
    /// Asserted via the pure helper so the owner-scope is provable without a
    /// live DB / registry.
    #[test]
    fn test14_emit_uses_office_document_owner_audience_and_stable_id() {
        let user = Uuid::new_v4();
        let full_name = r"C:\Users\test\Report.docx";

        let (entity, action, id, audience) =
            office_document_emit(user, SyncAction::Create, full_name);

        assert_eq!(entity, SyncEntity::OfficeDocument);
        assert_eq!(action, SyncAction::Create);
        // Owner-scope (DEC-7): delivered ONLY to the owning user's connections.
        match audience {
            Audience::Owner(uid) => assert_eq!(uid, user, "audience owner == target user"),
            other => panic!("expected Audience::Owner, got {other:?}"),
        }
        // Stable v5 id: same full_name → same id (open + close address one
        // entity); different full_name → different id.
        assert_eq!(id, office_document_id(full_name), "id is full_name-derived");
        assert_eq!(
            id,
            office_document_emit(user, SyncAction::Delete, full_name).2,
            "close of the same doc reuses the open's entity id"
        );
        assert_ne!(
            id,
            office_document_id(r"C:\Users\test\Other.docx"),
            "distinct documents get distinct ids"
        );
    }

    /// The serialized entity name matches the frontend `sync:<entity>`
    /// vocabulary (`office_document`), so the panel store's subscription key is
    /// derivable on the next OpenAPI regen (ITEM-15).
    #[test]
    fn test14_office_document_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&SyncEntity::OfficeDocument).unwrap(),
            "\"office_document\""
        );
    }
}
