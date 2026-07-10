//! Generic Server-Sent-Events driver — the RESPONSE side of the provider
//! adapter.
//!
//! Every provider's streaming response used to hand-roll the same scaffolding:
//! an incremental UTF-8 decoder, a rolling buffer split on an event delimiter, a
//! `MAX_SSE_BUFFER_BYTES` overflow guard, and byte-stream error mapping. That
//! loop is centralized here; each provider supplies only a small `SseAdapter`
//! (its event delimiter + a `map_event` that turns one raw event block into
//! `StreamChatChunk`s, using its own per-stream `State`).

use std::ops::ControlFlow;

use futures_util::{Stream, StreamExt};

use super::{Utf8StreamDecoder, MAX_SSE_BUFFER_BYTES};
use crate::error::ProviderError;
use crate::models::StreamChatChunk;

/// What a provider's `map_event` yields for a single SSE event block: any number
/// of result items to emit downstream, plus a control signal.
pub(crate) struct EventOutcome {
    /// Items to yield for this event (deltas / usage / finish, or an error).
    pub items: Vec<Result<StreamChatChunk, ProviderError>>,
    /// `Continue` ⇒ keep draining events from the buffer; `Break` ⇒ stop draining
    /// the current buffer (e.g. `[DONE]`, an in-stream error, a prompt block).
    pub control: ControlFlow<()>,
}

impl EventOutcome {
    /// Nothing to emit; keep going.
    pub(crate) fn empty() -> Self {
        Self {
            items: Vec::new(),
            control: ControlFlow::Continue(()),
        }
    }
    /// Emit `items`; keep going.
    pub(crate) fn emit(items: Vec<Result<StreamChatChunk, ProviderError>>) -> Self {
        Self {
            items,
            control: ControlFlow::Continue(()),
        }
    }
    /// Emit `items` then stop draining the current buffer.
    pub(crate) fn emit_then_break(items: Vec<Result<StreamChatChunk, ProviderError>>) -> Self {
        Self {
            items,
            control: ControlFlow::Break(()),
        }
    }
}

/// Per-provider mapping from raw SSE events to unified chunks.
pub(crate) trait SseAdapter {
    /// Per-stream accumulator state (usage totals, index-freeze, …).
    type State: Default + Send;

    /// Event delimiters, tried in order — the first that matches anywhere in the
    /// buffer wins (so Gemini's `\r\n\r\n` is preferred over a stray `\n\n`).
    fn delimiters(&self) -> &'static [&'static str];

    /// Provider label used in the buffer-overflow error message.
    fn label(&self) -> &'static str;

    /// Map one raw event block (the text between two delimiters) into chunks.
    fn map_event(&self, event: &str, state: &mut Self::State) -> EventOutcome;
}

/// Find + remove the next event block from `buffer`, trying `delimiters` in
/// order. Returns the block text (delimiter stripped).
fn take_event(buffer: &mut String, delimiters: &[&str]) -> Option<String> {
    for d in delimiters {
        if let Some(index) = buffer.find(d) {
            let event = buffer[..index].to_string();
            buffer.drain(..index + d.len());
            return Some(event);
        }
    }
    None
}

/// Drive a provider byte stream into a unified `StreamChatChunk` stream using
/// `adapter`. Owns the decode/buffer/split/overflow/network scaffolding.
pub(crate) fn drive_sse<A, S, B>(
    byte_stream: S,
    adapter: A,
) -> impl Stream<Item = Result<StreamChatChunk, ProviderError>> + Send
where
    A: SseAdapter + Send + 'static,
    S: Stream<Item = Result<B, reqwest::Error>> + Send + 'static,
    B: AsRef<[u8]> + Send,
{
    async_stream::stream! {
        let mut buffer = String::new();
        let mut decoder = Utf8StreamDecoder::default();
        let mut state = A::State::default();
        let mut byte_stream = Box::pin(byte_stream);

        while let Some(chunk_result) = byte_stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    // Decode incrementally so a multi-byte UTF-8 char split
                    // across chunk boundaries doesn't abort the stream.
                    buffer.push_str(&decoder.decode(chunk.as_ref()));

                    while let Some(event) = take_event(&mut buffer, adapter.delimiters()) {
                        let outcome = adapter.map_event(&event, &mut state);
                        for item in outcome.items {
                            yield item;
                        }
                        if outcome.control.is_break() {
                            break;
                        }
                    }

                    // Guard against an upstream that never emits an event
                    // delimiter (would otherwise grow `buffer` until OOM).
                    if buffer.len() > MAX_SSE_BUFFER_BYTES {
                        yield Err(ProviderError::streaming(format!(
                            "{}: SSE buffer exceeded maximum size",
                            adapter.label()
                        )));
                        break;
                    }
                }
                Err(e) => {
                    yield Err(ProviderError::Network(e));
                    break;
                }
            }
        }
    }
}

/// Extract the `data:` payload from a single-line SSE event block
/// (`data: <payload>`), or `None` if the block has no data line. Shared by the
/// OpenAI/Gemini adapters (their events are a single `data:` line).
pub(crate) fn single_data_line(event: &str) -> Option<&str> {
    event.strip_prefix("data: ")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoAdapter;
    #[derive(Default)]
    struct Count(usize);
    impl SseAdapter for EchoAdapter {
        type State = Count;
        fn delimiters(&self) -> &'static [&'static str] {
            &["\n\n"]
        }
        fn label(&self) -> &'static str {
            "Echo"
        }
        fn map_event(&self, event: &str, state: &mut Count) -> EventOutcome {
            if let Some(data) = single_data_line(event) {
                if data == "[DONE]" {
                    return EventOutcome {
                        items: Vec::new(),
                        control: ControlFlow::Break(()),
                    };
                }
                state.0 += 1;
                EventOutcome::emit(vec![Ok(StreamChatChunk {
                    content: vec![crate::models::ContentBlockDelta::TextDelta {
                        index: 0,
                        delta: data.to_string(),
                    }],
                    finish_reason: None,
                    usage: None,
                    refusal: None,
                    safety_ratings: Vec::new(),
                    safety_blocked: false,
                })])
            } else {
                EventOutcome::empty()
            }
        }
    }

    #[tokio::test]
    async fn driver_splits_and_maps_events_across_chunk_boundaries() {
        // Feed the SSE across arbitrary byte-chunk boundaries.
        let bytes: Vec<Result<Vec<u8>, reqwest::Error>> = vec![
            Ok(b"data: hel".to_vec()),
            Ok(b"lo\n\ndata: wor".to_vec()),
            Ok(b"ld\n\ndata: [DONE]\n\n".to_vec()),
        ];
        let stream = futures_util::stream::iter(bytes);
        let out: Vec<_> = drive_sse(stream, EchoAdapter).collect().await;
        let texts: Vec<String> = out
            .into_iter()
            .filter_map(|r| r.ok())
            .flat_map(|c| c.content)
            .filter_map(|d| match d {
                crate::models::ContentBlockDelta::TextDelta { delta, .. } => Some(delta),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["hello".to_string(), "world".to_string()]);
    }
}
