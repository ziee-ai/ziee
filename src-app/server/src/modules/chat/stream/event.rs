//! Wire types for the per-user chat-token stream (`GET /api/chat/stream`).
//!
//! Unlike the notify-only `sync` stream, this channel carries PAYLOADS: live
//! assistant generation frames. Each frame REUSES the existing per-request
//! event vocabulary (`SSEChatStreamEvent` — `started`/`content`/`complete`/
//! `error` + extension variants) and just wraps it in a thin envelope that
//! ALWAYS carries the `conversation_id` routing key, so one multiplexed stream
//! can serve every conversation the user views.

use axum::response::sse::Event;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::modules::chat::core::types::streaming::SSEChatStreamEvent;

/// Handshake frame opening the stream: the server-assigned connection id, which
/// the client echoes on `PUT /api/chat/stream/subscription` to scope delivery.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamConnectedData {
    pub connection_id: Uuid,
}

/// One generation frame, tagged with the conversation it belongs to. The inner
/// `event` is the unchanged per-request `SSEChatStreamEvent`; the envelope only
/// adds the routing key (today's `ChatStreamChunk.conversation_id` is sent only
/// on the first content chunk, which is ambiguous on a multiplexed stream).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamFrame {
    pub conversation_id: Uuid,
    pub event: SSEChatStreamEvent,
}

impl ChatStreamFrame {
    pub fn new(conversation_id: Uuid, event: SSEChatStreamEvent) -> Self {
        Self {
            conversation_id,
            event,
        }
    }

    /// True for the terminal frames (`complete`/`error`) — the buffer is
    /// dropped after one of these.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.event.event_name(),
            "complete" | "error"
        )
    }

    /// True for the opening `started` frame — creates the replay buffer.
    pub fn is_started(&self) -> bool {
        self.event.event_name() == "started"
    }

    /// Build the SSE wire `Event`. The `event:` line keeps the INNER event name
    /// (`started`/`content`/…) so a client can dispatch by name exactly as on
    /// the per-request stream; the `data:` line is `{conversationId, event}`.
    pub fn to_sse(&self) -> Event {
        Event::default()
            .event(self.event.event_name())
            .data(serde_json::to_string(self).unwrap_or_default())
    }
}

/// The opening handshake as an SSE `Event` (`event: connected`).
pub fn connected_event(connection_id: Uuid) -> Event {
    Event::default().event("connected").data(
        serde_json::to_string(&ChatStreamConnectedData { connection_id }).unwrap_or_default(),
    )
}

/// A documentation-only union of everything that can cross the stream, so the
/// frame + handshake shapes surface in the generated OpenAPI/TS types. The
/// real wire frames are built via [`ChatStreamFrame::to_sse`] /
/// [`connected_event`] (which keep the inner event name on the `event:` line).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatStreamSseEvent {
    Connected(ChatStreamConnectedData),
    Frame(ChatStreamFrame),
}
