//! Chat-token streaming.
//!
//! A per-user SSE stream (`GET /api/chat/stream`) carrying live assistant
//! generation frames (PAYLOADS — distinct from the notify-only `sync` stream),
//! scoped to the one conversation each connection is viewing via
//! `PUT /api/chat/stream/subscription`, with a per-conversation replay buffer
//! for seamless mid-stream join. The detached generation task calls
//! [`publish_frame`] for every chunk + the terminal frame.

pub mod buffers;
pub mod event;
pub mod handler;
pub mod registry;

pub use event::ChatStreamFrame;
pub use handler::chat_stream_router;
pub use registry::{begin_generation, end_generation, publish_frame, publish_raw_event};
