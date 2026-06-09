//! Per-generation replay buffer for seamless mid-stream join.
//!
//! While a turn is generating, every frame is appended here (keyed by
//! conversation id in the registry). When a device subscribes to that
//! conversation mid-stream — opening it, returning to it, reconnecting, or a
//! second device joining — the registry atomically replays the buffered frames
//! so the client renders the FULL reply-so-far, then continues live. The
//! buffer is dropped on the terminal `complete`/`error` frame, after which the
//! finalized message is read from the DB.

use std::collections::VecDeque;

use super::event::ChatStreamFrame;

/// Soft cap on a single generation's buffered bytes. A reply that exceeds this
/// (tens of thousands of tokens) evicts its OLDEST non-`started` frames, so a
/// catch-up of a giant reply may begin slightly in — acceptable; the terminal
/// `complete` + DB refetch still lands the whole message. Normal replies stay
/// well under this and replay in full.
const MAX_BUFFER_BYTES: usize = 128 * 1024;

/// The ordered frames of one in-flight turn. `started` is held separately so it
/// is NEVER evicted (it carries `assistant_message_id` the client needs to seed
/// the message); the byte cap evicts only from the middle/old content frames.
#[derive(Default)]
pub struct GenerationBuffer {
    started: Option<ChatStreamFrame>,
    frames: VecDeque<(ChatStreamFrame, usize)>,
    bytes: usize,
}

impl GenerationBuffer {
    /// Append a frame to the buffer (called for every non-terminal frame).
    pub fn push(&mut self, frame: ChatStreamFrame) {
        if frame.is_started() {
            self.started = Some(frame);
            return;
        }
        let size = approx_size(&frame);
        self.frames.push_back((frame, size));
        self.bytes += size;
        while self.bytes > MAX_BUFFER_BYTES && self.frames.len() > 1 {
            if let Some((_, evicted)) = self.frames.pop_front() {
                self.bytes -= evicted;
            }
        }
    }

    /// The frames to replay to a newly-subscribed connection, in order:
    /// `started` (if seen) followed by the retained content frames.
    pub fn replay(&self) -> Vec<ChatStreamFrame> {
        let mut out = Vec::with_capacity(self.frames.len() + 1);
        if let Some(started) = &self.started {
            out.push(started.clone());
        }
        out.extend(self.frames.iter().map(|(f, _)| f.clone()));
        out
    }
}

/// Rough serialized size of a frame, for the byte cap. Cheap to compute and
/// only needs to be proportional, not exact.
fn approx_size(frame: &ChatStreamFrame) -> usize {
    serde_json::to_string(frame).map(|s| s.len()).unwrap_or(0)
}
