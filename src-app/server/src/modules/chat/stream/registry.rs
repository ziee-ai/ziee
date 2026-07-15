//! In-process, per-user registry for the chat-token stream.
//!
//! Like the `sync` registry it is keyed by user (a generation's frames reach
//! only the owning user's connections) — but delivery is further SCOPED to the
//! one conversation each connection is currently subscribed to, and it owns a
//! per-conversation replay buffer for seamless mid-stream join. Single-process
//! today; a multi-instance deployment would fan out via LISTEN/NOTIFY.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use axum::http::StatusCode;
use axum::response::sse::Event;
use lazy_static::lazy_static;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::common::AppError;

use super::buffers::GenerationBuffer;
use super::event::ChatStreamFrame;

/// Chat-token SSE connection caps (DEC-34). Formerly two hardcoded consts
/// (`GLOBAL_MAX_CONNECTIONS`/`PER_USER_MAX_CONNECTIONS = 12`); now a Limits struct
/// so the caps are deployment-config-driven (`chat.*` in `Config`) and unit-test
/// injectable. The per-user default is raised (24) because split-chat opens one
/// dedicated SSE connection per open pane.
#[derive(Debug, Clone, Copy)]
pub struct ChatStreamLimits {
    /// Max concurrent connections for a single user (all tabs/devices/panes).
    pub per_user_max_connections: usize,
    /// Max concurrent connections across all users.
    pub global_max_connections: usize,
}

impl Default for ChatStreamLimits {
    fn default() -> Self {
        Self {
            per_user_max_connections: 24,
            global_max_connections: 512,
        }
    }
}

/// Per-connection queue depth. Sized to comfortably hold a full catch-up replay
/// (the byte-capped buffer is well under this many frames) plus live headroom.
/// A reader that falls this far behind is dropped (client reconnects + replays).
pub(crate) const CHAT_STREAM_CHANNEL_CAPACITY: usize = 2048;

type ConnId = Uuid;

/// One live chat-token connection. `active_conversation` is the conversation
/// whose frames this connection currently wants (set via the subscription PUT);
/// `None` means "subscribed to nothing" — receives no frames.
pub struct ChatConn {
    pub user_id: Uuid,
    pub active_conversation: Option<Uuid>,
    pub sender: Sender<Result<Event, axum::Error>>,
}

struct RegistryInner {
    clients: HashMap<ConnId, ChatConn>,
    by_user: HashMap<Uuid, HashSet<ConnId>>,
    /// In-flight generations keyed by conversation id (one turn per
    /// conversation at a time). Created on `started`, dropped on terminal.
    generations: HashMap<Uuid, GenerationBuffer>,
    /// Connection caps (DEC-34). Defaults applied at construction; overridden at
    /// server boot from deployment config via `set_limits`.
    limits: ChatStreamLimits,
}

pub struct ChatStreamRegistry {
    inner: Mutex<RegistryInner>,
}

lazy_static! {
    static ref REGISTRY: ChatStreamRegistry = ChatStreamRegistry {
        inner: Mutex::new(RegistryInner {
            clients: HashMap::new(),
            by_user: HashMap::new(),
            generations: HashMap::new(),
            limits: ChatStreamLimits::default(),
        }),
    };
}

/// Process-wide singleton registry.
pub fn registry() -> &'static ChatStreamRegistry {
    &REGISTRY
}

impl ChatStreamRegistry {
    /// Register a new connection. 429 on a global or per-user cap.
    pub fn register(&self, conn_id: ConnId, conn: ChatConn) -> Result<(), AppError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        if inner.clients.len() >= inner.limits.global_max_connections {
            return Err(AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "CHAT_STREAM_GLOBAL_LIMIT",
                "Chat streaming is at capacity; retry shortly",
            ));
        }
        let user_count = inner.by_user.get(&conn.user_id).map_or(0, |s| s.len());
        if user_count >= inner.limits.per_user_max_connections {
            return Err(AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "CHAT_STREAM_USER_LIMIT",
                "Too many open chat-stream connections for this account",
            ));
        }

        inner.by_user.entry(conn.user_id).or_default().insert(conn_id);
        inner.clients.insert(conn_id, conn);
        Ok(())
    }

    /// Remove a connection (called on stream termination).
    pub fn unregister(&self, conn_id: ConnId) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        remove_conn(&mut inner, conn_id);
    }

    /// Override the connection caps (called once at server boot from deployment
    /// config — DEC-34). Affects only subsequent `register` calls.
    pub fn set_limits(&self, limits: ChatStreamLimits) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.limits = limits;
    }

    /// Claim the single in-flight generation slot for a conversation. Returns
    /// `false` if one is already running — the caller must reject the send
    /// (409) so two concurrent turns can't interleave into one replay buffer
    /// (which carries no message id to demux them). Creates the (empty) buffer.
    ///
    /// The slot is released when the generation emits a terminal frame (or via
    /// `end_generation` on setup failure / `TerminalGuard` on panic). The only
    /// path that leaves it stuck is the runtime dropping the spawned generation
    /// task *before its first poll* (i.e. process shutdown) — in-process
    /// unrecoverable, but the process is exiting, so accepted.
    pub fn begin_generation(&self, conversation_id: Uuid) -> bool {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.generations.contains_key(&conversation_id) {
            return false;
        }
        inner
            .generations
            .insert(conversation_id, GenerationBuffer::default());
        true
    }

    /// Read-only: is a generation currently in flight for this conversation?
    /// The slot is dropped when the terminal (`complete`/`error`) frame is
    /// published, so this flips false exactly when the turn finishes — the
    /// signal background code (e.g. the scheduler's prompt dispatch) polls to
    /// know a detached turn has completed. Does NOT claim the slot.
    pub fn is_generating(&self, conversation_id: Uuid) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.generations.contains_key(&conversation_id)
    }

    /// Release the in-flight slot WITHOUT delivering a terminal frame — used
    /// only when generation setup fails before the streaming loop starts.
    /// (The normal path drops the buffer via the terminal frame in
    /// `publish_frame`.)
    pub fn end_generation(&self, conversation_id: Uuid) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.generations.remove(&conversation_id);
    }

    /// Point a connection at a conversation (or `None` to receive nothing).
    /// Atomically replays that conversation's in-flight reply-so-far to this
    /// connection — the catch-up. Holding the single registry lock means no
    /// live `publish_frame` can interleave between the snapshot and the
    /// connection becoming eligible, so there is no gap and no duplicate.
    pub fn set_subscription(&self, conn_id: ConnId, conversation_id: Option<Uuid>) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        let Some(conn) = inner.clients.get_mut(&conn_id) else {
            return;
        };
        conn.active_conversation = conversation_id;

        // Catch-up: replay the in-flight frames for the newly-subscribed
        // conversation, if any generation is active.
        let replay = match conversation_id {
            Some(cid) => inner
                .generations
                .get(&cid)
                .map(|b| b.replay())
                .unwrap_or_default(),
            None => Vec::new(),
        };
        if replay.is_empty() {
            return;
        }
        let mut dead = false;
        if let Some(conn) = inner.clients.get(&conn_id) {
            for frame in &replay {
                if conn.sender.try_send(Ok(frame.to_sse())).is_err() {
                    dead = true;
                    break;
                }
            }
        }
        if dead {
            remove_conn(&mut inner, conn_id);
        }
    }

    /// Append a generation frame to its conversation's replay buffer and deliver
    /// it to the owner's connections currently subscribed to that conversation.
    /// On a terminal frame (`complete`/`error`) the buffer is dropped after
    /// delivery. A connection whose bounded queue is full/closed is pruned.
    pub fn publish_frame(&self, owner_id: Uuid, frame: ChatStreamFrame) {
        let conversation_id = frame.conversation_id;
        let terminal = frame.is_terminal();
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        // Buffer (non-terminal frames only; terminal drops the buffer below).
        if !terminal {
            inner
                .generations
                .entry(conversation_id)
                .or_default()
                .push(frame.clone());
        }

        // Deliver to the owner's connections subscribed to this conversation.
        let sse = frame.to_sse();
        let mut dead: Vec<ConnId> = Vec::new();
        if let Some(set) = inner.by_user.get(&owner_id) {
            for cid in set {
                if let Some(conn) = inner.clients.get(cid) {
                    if conn.active_conversation == Some(conversation_id)
                        && conn.sender.try_send(Ok(sse.clone())).is_err()
                    {
                        dead.push(*cid);
                    }
                }
            }
        }

        if terminal {
            inner.generations.remove(&conversation_id);
        }
        for cid in dead {
            remove_conn(&mut inner, cid);
        }
    }

    /// Deliver a pre-built extension SSE `Event` (titleUpdated / MCP tool
    /// lifecycle, approval, elicitation, artifact) to the owner's connections
    /// subscribed to `conversation_id`. Unlike `publish_frame` these are NOT
    /// enveloped or buffered for replay — they're raw events; the client routes
    /// them to whatever conversation the connection is currently subscribed to.
    pub fn publish_raw_event(&self, owner_id: Uuid, conversation_id: Uuid, event: Event) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut dead: Vec<ConnId> = Vec::new();
        if let Some(set) = inner.by_user.get(&owner_id) {
            for cid in set {
                if let Some(conn) = inner.clients.get(cid) {
                    if conn.active_conversation == Some(conversation_id)
                        && conn.sender.try_send(Ok(event.clone())).is_err()
                    {
                        dead.push(*cid);
                    }
                }
            }
        }
        for cid in dead {
            remove_conn(&mut inner, cid);
        }
    }

}

/// Publish one generation frame to the owner's subscribed connections (and the
/// conversation's replay buffer). Called by the detached generation task for
/// every chunk + the terminal frame. There is no origin-suppression: the sender
/// no longer gets tokens from its send response, so it consumes its own
/// generation over this stream like every other subscribed device.
pub fn publish_frame(owner_id: Uuid, frame: ChatStreamFrame) {
    registry().publish_frame(owner_id, frame);
}

/// Claim the single in-flight generation slot for a conversation (see
/// [`ChatStreamRegistry::begin_generation`]).
pub fn begin_generation(conversation_id: Uuid) -> bool {
    registry().begin_generation(conversation_id)
}

/// Read-only: is a generation in flight for this conversation? (see
/// [`ChatStreamRegistry::is_generating`]). Flips false when the turn finishes.
pub fn is_generating(conversation_id: Uuid) -> bool {
    registry().is_generating(conversation_id)
}

/// Deliver a raw extension SSE event to subscribers of a conversation (see
/// [`ChatStreamRegistry::publish_raw_event`]).
pub fn publish_raw_event(owner_id: Uuid, conversation_id: Uuid, event: Event) {
    registry().publish_raw_event(owner_id, conversation_id, event);
}

/// Release a generation slot reserved by [`begin_generation`] without a
/// terminal frame (setup-failure path only).
pub fn end_generation(conversation_id: Uuid) {
    registry().end_generation(conversation_id);
}

/// Apply the deployment-config chat-stream connection caps to the process-wide
/// registry. Call once at server boot (before serving) — DEC-34.
pub fn apply_config_limits(chat: &crate::core::config::ChatConfig) {
    registry().set_limits(ChatStreamLimits {
        per_user_max_connections: chat.per_user_max_connections,
        global_max_connections: chat.global_max_connections,
    });
}

/// Remove a connection from both indexes (shared by unregister + pruning).
fn remove_conn(inner: &mut RegistryInner, conn_id: ConnId) {
    if let Some(conn) = inner.clients.remove(&conn_id) {
        if let Some(set) = inner.by_user.get_mut(&conn.user_id) {
            set.remove(&conn_id);
            if set.is_empty() {
                inner.by_user.remove(&conn.user_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::chat::core::types::streaming::{
        ChatStreamChunk, ContentBlockDelta, SSEChatStreamCompleteData, SSEChatStreamEvent,
        SSEChatStreamStartedData,
    };
    use tokio::sync::mpsc::Receiver;

    type Rx = Receiver<Result<Event, axum::Error>>;

    fn empty_registry() -> ChatStreamRegistry {
        ChatStreamRegistry {
            inner: Mutex::new(RegistryInner {
                clients: HashMap::new(),
                by_user: HashMap::new(),
                generations: HashMap::new(),
                limits: ChatStreamLimits::default(),
            }),
        }
    }

    /// A connection for `user_id` subscribed to `active` (None = nothing).
    fn conn(user_id: Uuid, active: Option<Uuid>) -> (ChatConn, Rx) {
        let (tx, rx) = tokio::sync::mpsc::channel(CHAT_STREAM_CHANNEL_CAPACITY);
        (
            ChatConn {
                user_id,
                active_conversation: active,
                sender: tx,
            },
            rx,
        )
    }

    fn started(conv: Uuid) -> ChatStreamFrame {
        ChatStreamFrame::new(
            conv,
            SSEChatStreamEvent::Started(SSEChatStreamStartedData {
                user_message_id: None,
                conversation_id: conv,
                branch_id: Uuid::new_v4(),
            }),
        )
    }

    fn content(conv: Uuid, text: &str) -> ChatStreamFrame {
        ChatStreamFrame::new(
            conv,
            SSEChatStreamEvent::Content(ChatStreamChunk {
                content: vec![ContentBlockDelta::TextDelta {
                    index: 0,
                    delta: text.to_string(),
                }],
                ..Default::default()
            }),
        )
    }

    fn complete(conv: Uuid) -> ChatStreamFrame {
        ChatStreamFrame::new(
            conv,
            SSEChatStreamEvent::Complete(SSEChatStreamCompleteData {
                finish_reason: "stop".into(),
                usage: None,
            }),
        )
    }

    /// Drain a receiver, counting delivered frames.
    fn drain(rx: &mut Rx) -> usize {
        let mut n = 0;
        while rx.try_recv().is_ok() {
            n += 1;
        }
        n
    }

    #[test]
    fn delivery_is_scoped_to_the_subscribed_conversation_and_user() {
        let reg = empty_registry();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let conv_x = Uuid::new_v4();
        let conv_y = Uuid::new_v4();

        // A subscribed to X; A subscribed to Y; A subscribed to nothing; B subscribed to X.
        let (ax, mut rx_ax) = conn(a, Some(conv_x));
        let (ay, mut rx_ay) = conn(a, Some(conv_y));
        let (anone, mut rx_anone) = conn(a, None);
        let (bx, mut rx_bx) = conn(b, Some(conv_x));
        reg.register(Uuid::new_v4(), ax).unwrap();
        reg.register(Uuid::new_v4(), ay).unwrap();
        reg.register(Uuid::new_v4(), anone).unwrap();
        reg.register(Uuid::new_v4(), bx).unwrap();

        // A turn in X, owned by A.
        reg.publish_frame(a, started(conv_x));
        reg.publish_frame(a, content(conv_x, "hi"));

        assert_eq!(drain(&mut rx_ax), 2, "A's conn subscribed to X receives X's frames");
        assert_eq!(drain(&mut rx_ay), 0, "A's conn on a different conversation gets nothing");
        assert_eq!(drain(&mut rx_anone), 0, "A's unsubscribed conn gets nothing");
        assert_eq!(drain(&mut rx_bx), 0, "user B never receives user A's frames");
    }

    #[test]
    fn subscribing_mid_generation_replays_the_reply_so_far() {
        let reg = empty_registry();
        let a = Uuid::new_v4();
        let conv = Uuid::new_v4();

        // A generation is already in flight (no subscriber yet).
        reg.publish_frame(a, started(conv));
        reg.publish_frame(a, content(conv, "Hel"));
        reg.publish_frame(a, content(conv, "lo"));

        // A device opens the conversation mid-stream.
        let (c, mut rx) = conn(a, None);
        let cid = Uuid::new_v4();
        reg.register(cid, c).unwrap();
        reg.set_subscription(cid, Some(conv));

        // Catch-up replays started + both content frames.
        assert_eq!(drain(&mut rx), 3, "catch-up replays the buffered reply-so-far");

        // A subsequent live frame continues with no gap/dup.
        reg.publish_frame(a, content(conv, "!"));
        assert_eq!(drain(&mut rx), 1, "live frames continue after the catch-up");
    }

    #[test]
    fn terminal_frame_drops_the_buffer_so_a_later_joiner_gets_no_replay() {
        let reg = empty_registry();
        let a = Uuid::new_v4();
        let conv = Uuid::new_v4();

        reg.publish_frame(a, started(conv));
        reg.publish_frame(a, content(conv, "done"));
        reg.publish_frame(a, complete(conv));

        let (c, mut rx) = conn(a, None);
        let cid = Uuid::new_v4();
        reg.register(cid, c).unwrap();
        reg.set_subscription(cid, Some(conv));

        assert_eq!(
            drain(&mut rx),
            0,
            "after complete the buffer is gone; the finished message comes from the DB"
        );
    }

    #[test]
    fn unsubscribe_with_null_stops_delivery() {
        let reg = empty_registry();
        let a = Uuid::new_v4();
        let conv = Uuid::new_v4();
        let (c, mut rx) = conn(a, Some(conv));
        let cid = Uuid::new_v4();
        reg.register(cid, c).unwrap();

        reg.set_subscription(cid, None);
        reg.publish_frame(a, started(conv));
        assert_eq!(drain(&mut rx), 0, "a conn unsubscribed (null) receives nothing");
    }

    #[test]
    fn per_user_cap_rejects_excess_connections() {
        let reg = empty_registry();
        let cap = ChatStreamLimits::default().per_user_max_connections;
        let uid = Uuid::new_v4();
        for _ in 0..cap {
            let (c, _rx) = conn(uid, None);
            reg.register(Uuid::new_v4(), c).unwrap();
        }
        let (overflow, _rx) = conn(uid, None);
        assert!(
            reg.register(Uuid::new_v4(), overflow).is_err(),
            "the (cap+1)th connection for one user must be refused (429)"
        );
    }

    /// TEST-36: the per-user cap reads the CONFIGURED value (via `set_limits`),
    /// not the legacy hardcoded 12, and the (cap+1)th connection 429s at the
    /// configured bound (DEC-34 / ITEM-20).
    #[test]
    fn per_user_cap_honors_the_configured_limit() {
        let reg = empty_registry();
        // A configured cap distinct from both the legacy 12 and the new default 24.
        reg.set_limits(ChatStreamLimits {
            per_user_max_connections: 3,
            global_max_connections: 512,
        });
        let uid = Uuid::new_v4();
        for _ in 0..3 {
            let (c, _rx) = conn(uid, None);
            reg.register(Uuid::new_v4(), c).unwrap();
        }
        let (overflow, _rx) = conn(uid, None);
        assert!(
            reg.register(Uuid::new_v4(), overflow).is_err(),
            "the 4th connection must 429 at the CONFIGURED cap of 3"
        );
    }

    /// The default per-user cap was raised above the legacy 12 (DEC-34) so
    /// one-SSE-connection-per-pane doesn't 429 a legitimate pane under churn.
    #[test]
    fn default_per_user_cap_is_raised_above_legacy_twelve() {
        assert!(
            ChatStreamLimits::default().per_user_max_connections > 12,
            "the default per-user cap must exceed the legacy 12"
        );
    }

    #[test]
    fn begin_generation_is_exclusive_per_conversation() {
        let reg = empty_registry();
        let conv = Uuid::new_v4();
        assert!(reg.begin_generation(conv), "first claim succeeds");
        assert!(
            !reg.begin_generation(conv),
            "a second concurrent generation for the same conversation is refused"
        );
        // A terminal frame drops the buffer (releases the slot)…
        reg.publish_frame(Uuid::new_v4(), complete(conv));
        assert!(
            reg.begin_generation(conv),
            "after the turn terminates, a new generation can begin"
        );
        // …and end_generation releases it without a terminal frame.
        reg.end_generation(conv);
        assert!(reg.begin_generation(conv));
    }

    #[test]
    fn raw_extension_events_are_scoped_and_not_buffered() {
        let reg = empty_registry();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let conv = Uuid::new_v4();

        let (ax, mut rx_ax) = conn(a, Some(conv));
        let (anone, mut rx_anone) = conn(a, None);
        let (bx, mut rx_bx) = conn(b, Some(conv));
        reg.register(Uuid::new_v4(), ax).unwrap();
        reg.register(Uuid::new_v4(), anone).unwrap();
        reg.register(Uuid::new_v4(), bx).unwrap();

        let raw = Event::default().event("titleUpdated").data("{}");
        reg.publish_raw_event(a, conv, raw);

        assert_eq!(drain(&mut rx_ax), 1, "A's conn subscribed to the conversation receives it");
        assert_eq!(drain(&mut rx_anone), 0, "A's unsubscribed conn does not");
        assert_eq!(drain(&mut rx_bx), 0, "a different user never receives it");

        // Raw events are NOT buffered: a connection that subscribes AFTER the
        // event gets no replay (only the chunk-derived frames are buffered).
        let (late, mut rx_late) = conn(a, None);
        let late_id = Uuid::new_v4();
        reg.register(late_id, late).unwrap();
        reg.set_subscription(late_id, Some(conv));
        assert_eq!(drain(&mut rx_late), 0, "a late joiner gets no raw-event replay");
    }
}
