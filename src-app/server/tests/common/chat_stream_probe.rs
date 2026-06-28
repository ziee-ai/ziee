//! Test helper: a client-side probe for the per-user chat-token SSE stream.
//!
//! `ChatStreamProbe::open` subscribes to `GET /api/chat/stream` exactly like a
//! real browser tab, captures the `connected` handshake's connection id, then
//! `subscribe(conversation_id)` PUTs `/api/chat/stream/subscription` (echoing
//! the connection id via `X-Chat-Stream-Connection-Id`) so the server scopes
//! delivery to that conversation. The reader decodes both frame shapes:
//!   * enveloped generation frames `{conversationId, event:{type,…}}`
//!     (started / content / complete / error);
//!   * raw extension events `{type,…}` (titleUpdated, mcpToolStart, …) which
//!     carry no conversation id (they belong to the subscribed conversation).
//!
//! It mirrors `sync_probe::SyncProbe`, but the chat stream is camelCase
//! (`connectionId`, `conversationId`) and is conversation-scoped, not a flat
//! per-user notify feed.

use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use uuid::Uuid;

/// One decoded chat-stream frame.
#[derive(Debug, Clone)]
pub struct ChatFrame {
    /// The envelope's `conversationId` (None for raw extension events).
    pub conversation_id: Option<String>,
    /// The inner event's `type` (started / content / complete / error /
    /// mcpToolStart / titleUpdated / …).
    pub event_type: String,
    /// The inner event object (the `event` of an envelope, or the raw event).
    pub data: serde_json::Value,
}

impl ChatFrame {
    pub fn is_terminal(&self) -> bool {
        self.event_type == "complete" || self.event_type == "error"
    }

    /// True if this frame is for `conversation_id`. Raw extension events
    /// (`titleUpdated`, `mcpToolStart`, …) carry no `conversationId` — but the
    /// server only delivers them to a connection subscribed to THIS
    /// conversation, so a `None` is treated as "mine".
    fn belongs_to(&self, conversation_id: &str) -> bool {
        match &self.conversation_id {
            Some(id) => id == conversation_id,
            None => true,
        }
    }

    /// Concatenate `text_delta` deltas out of a `content` frame's `content[]`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        if let Some(arr) = self.data.get("content").and_then(|c| c.as_array()) {
            for block in arr {
                if block.get("type").and_then(|t| t.as_str()) == Some("text_delta") {
                    if let Some(d) = block.get("delta").and_then(|d| d.as_str()) {
                        out.push_str(d);
                    }
                }
            }
        }
        out
    }
}

/// A live subscription to the chat-token stream for one user/token. Dropping it
/// aborts the reader task, which drops the HTTP response → the server's
/// ConnGuard unregisters the connection.
pub struct ChatStreamProbe {
    connection_id: Uuid,
    api_base: String,
    token: String,
    rx: mpsc::UnboundedReceiver<ChatFrame>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for ChatStreamProbe {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl ChatStreamProbe {
    /// Open the stream for `token`. Resolves once the `connected` handshake
    /// frame arrives (so `connection_id()` / `subscribe` are usable).
    pub async fn open(server: &crate::common::TestServer, token: &str) -> ChatStreamProbe {
        let resp = reqwest::Client::new()
            .get(server.api_url("/chat/stream"))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("chat stream subscribe request failed");
        assert_eq!(
            resp.status(),
            200,
            "GET /chat/stream should return 200 for an authenticated user"
        );

        let (id_tx, id_rx) = oneshot::channel::<Uuid>();
        let (frame_tx, frame_rx) = mpsc::unbounded_channel::<ChatFrame>();

        let task = tokio::spawn(async move {
            let mut stream = resp.bytes_stream();
            let mut buf = String::new();
            let mut id_tx = Some(id_tx);
            while let Some(Ok(chunk)) = stream.next().await {
                buf.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = buf.find("\n\n") {
                    let frame: String = buf.drain(..pos + 2).collect();
                    let (event, data) = parse_sse_frame(&frame);
                    let value = data
                        .as_deref()
                        .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());

                    match event.as_deref() {
                        Some("connected") => {
                            if let Some(tx) = id_tx.take() {
                                if let Some(id) = value
                                    .as_ref()
                                    .and_then(|v| v.get("connectionId"))
                                    .and_then(|c| c.as_str())
                                    .and_then(|s| Uuid::parse_str(s).ok())
                                {
                                    let _ = tx.send(id);
                                }
                            }
                        }
                        Some(_) => {
                            let Some(value) = value else { continue };
                            // Enveloped generation frame vs raw extension event.
                            let f = if value.get("conversationId").is_some()
                                && value.get("event").is_some()
                            {
                                let inner = value.get("event").cloned().unwrap_or_default();
                                ChatFrame {
                                    conversation_id: value
                                        .get("conversationId")
                                        .and_then(|c| c.as_str())
                                        .map(str::to_string),
                                    event_type: inner
                                        .get("type")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or_default()
                                        .to_string(),
                                    data: inner,
                                }
                            } else {
                                ChatFrame {
                                    conversation_id: None,
                                    event_type: value
                                        .get("type")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or_default()
                                        .to_string(),
                                    data: value,
                                }
                            };
                            if frame_tx.send(f).is_err() {
                                return; // receiver gone
                            }
                        }
                        None => {} // keep-alive comments
                    }
                }
            }
        });

        let connection_id = tokio::time::timeout(Duration::from_secs(5), id_rx)
            .await
            .expect("timed out waiting for the chat-stream `connected` handshake")
            .expect("chat probe task ended before the handshake");

        ChatStreamProbe {
            connection_id,
            api_base: server.api_url(""),
            token: token.to_string(),
            rx: frame_rx,
            task,
        }
    }

    // connection_id() method removed — callers access the field directly.
    /// Scope this connection to `conversation_id` (or `None` to receive
    /// nothing). PUTs the subscription with the handshake connection id; the
    /// server replays the reply-so-far if the conversation is mid-generation.
    pub async fn subscribe(&self, conversation_id: Option<Uuid>) {
        let resp = reqwest::Client::new()
            .put(format!("{}/chat/stream/subscription", self.api_base))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("X-Chat-Stream-Connection-Id", self.connection_id.to_string())
            .json(&serde_json::json!({
                "conversation_id": conversation_id.map(|c| c.to_string()),
            }))
            .send()
            .await
            .expect("chat stream subscription PUT failed");
        assert!(
            resp.status().is_success() || resp.status() == 204,
            "subscription PUT should succeed, got {}",
            resp.status()
        );
    }

    /// Wait up to `timeout` for a frame for `conversation_id` whose
    /// `event_type` matches, ignoring others. Panics on timeout / stream close.
    pub async fn expect_event(
        &mut self,
        conversation_id: Uuid,
        event_type: &str,
        timeout: Duration,
    ) -> ChatFrame {
        let want = conversation_id.to_string();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, self.rx.recv()).await {
                Ok(Some(f)) if f.event_type == event_type && f.belongs_to(&want) => return f,
                Ok(Some(_)) => {}
                Ok(None) => panic!("chat stream closed while waiting for {event_type}"),
                Err(_) => panic!("timed out waiting for chat frame {event_type}"),
            }
        }
    }

    /// Collect frames for `conversation_id` until a terminal (complete/error)
    /// frame, returning all collected frames (including the terminal one).
    pub async fn collect_until_terminal(
        &mut self,
        conversation_id: Uuid,
        timeout: Duration,
    ) -> Vec<ChatFrame> {
        self.collect_until(conversation_id, &[], timeout).await
    }

    /// Collect frames for `conversation_id` until one whose `event_type` is in
    /// `stop_at` OR a terminal (complete/error), returning all collected frames
    /// (including the stopping one). Use `stop_at` for flows that pause
    /// mid-stream with no terminal until a separate action — e.g.
    /// `mcpApprovalRequired` / `mcpElicitationRequired`.
    pub async fn collect_until(
        &mut self,
        conversation_id: Uuid,
        stop_at: &[&str],
        timeout: Duration,
    ) -> Vec<ChatFrame> {
        let want = conversation_id.to_string();
        let deadline = tokio::time::Instant::now() + timeout;
        let mut frames = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, self.rx.recv()).await {
                Ok(Some(f)) if f.belongs_to(&want) => {
                    let stop = f.is_terminal() || stop_at.contains(&f.event_type.as_str());
                    frames.push(f);
                    if stop {
                        return frames;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => panic!("chat stream closed before a stop/terminal frame"),
                Err(_) => panic!(
                    "timed out collecting chat frames for {want}; got {} so far",
                    frames.len()
                ),
            }
        }
    }

    /// Assemble the full assistant reply text from a collected frame list.
    pub fn assemble_text(frames: &[ChatFrame]) -> String {
        frames.iter().map(|f| f.text()).collect()
    }

    /// Assert NO chat frame at all arrives within `dur` (scoping / isolation).
    pub async fn expect_silence(&mut self, dur: Duration) {
        match tokio::time::timeout(dur, self.rx.recv()).await {
            Ok(Some(f)) => panic!(
                "expected chat-stream silence but received {} (conv {:?})",
                f.event_type, f.conversation_id
            ),
            Ok(None) | Err(_) => {}
        }
    }
}

/// Pull `event:` + concatenated `data:` lines out of one raw SSE frame.
fn parse_sse_frame(frame: &str) -> (Option<String>, Option<String>) {
    let mut event = None;
    let mut data_lines: Vec<String> = Vec::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
        }
    }
    let data = if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    };
    (event, data)
}
