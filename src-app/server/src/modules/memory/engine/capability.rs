//! Capability guard for the memory generation paths (extraction +
//! summarization).
//!
//! Memory uses two *different* kinds of model that are not
//! interchangeable:
//!   - an **embedding** model (`capabilities.text_embedding`) that
//!     vectorizes memory text for retrieval, and
//!   - a **chat/generation** model that reads the conversation, decides
//!     what to remember, and writes rolling summaries.
//!
//! An embedding model is started by the local runtime
//! (`llm_local_runtime/auto_start.rs`) with llama.cpp's `--embeddings`
//! flag, which puts the context in pooling/embedding mode where logits
//! are **not** computed. Asking such a model to generate text returns
//! HTTP 500 `"the current context does not support logits
//! computation"`. Remote embedding models likewise have no chat
//! endpoint. So using an embedding model for extraction/summarization
//! always fails — we guard against it here with a clear, actionable
//! message instead of firing a doomed request and swallowing the 500.
//!
//! The inverse — using a non-embedding model where an embedding model
//! is required — is guarded by [`embedding_unsupported_reason`], the
//! symmetric counterpart to the positive `text_embedding` check the
//! embedding dispatcher performs at runtime (`dispatch.rs`).

use crate::modules::llm_model::models::ModelCapabilities;

/// Returns a human-readable reason if a model with these `caps` cannot
/// be used for text generation (memory extraction / summarization),
/// otherwise `None`.
///
/// We reject on `text_embedding == Some(true)` regardless of `chat`,
/// because the local runtime forces `--embeddings` on *any* model
/// flagged `text_embedding` — even a dual-flagged one would 500. We do
/// **not** require a positive `chat == Some(true)`, so normal chat
/// models that simply never set the flag keep working.
pub fn generation_unsupported_reason(name: &str, caps: &ModelCapabilities) -> Option<String> {
    if caps.text_embedding == Some(true) {
        return Some(format!(
            "model '{name}' is an embedding model (text_embedding capability) and cannot \
             generate text; configure a chat-capable model as the memory extraction model"
        ));
    }
    None
}

/// Returns a human-readable reason if a model with these `caps` cannot
/// be used to produce embeddings (the memory embedding model), otherwise
/// `None`.
///
/// Mirrors the positive `text_embedding` check in `dispatch.rs` (the
/// runtime embedding path) so the admin gets the same answer at config
/// time. A model that isn't flagged `text_embedding` has no `/embeddings`
/// endpoint (remote) or isn't started in `--embeddings` mode (local), so
/// embedding generation would fail.
pub fn embedding_unsupported_reason(name: &str, caps: &ModelCapabilities) -> Option<String> {
    if caps.text_embedding != Some(true) {
        return Some(format!(
            "model '{name}' is not flagged with the text_embedding capability and cannot \
             produce embeddings; configure an embedding model"
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(text_embedding: Option<bool>, chat: Option<bool>) -> ModelCapabilities {
        ModelCapabilities {
            text_embedding,
            chat,
            ..Default::default()
        }
    }

    #[test]
    fn embedding_only_model_is_rejected() {
        let reason = generation_unsupported_reason("nomic-embed", &caps(Some(true), None));
        let reason = reason.expect("embedding-only model must be rejected");
        assert!(reason.contains("nomic-embed"), "reason names the model: {reason}");
        assert!(reason.contains("embedding"), "reason mentions embedding: {reason}");
    }

    #[test]
    fn chat_model_is_allowed() {
        assert_eq!(
            generation_unsupported_reason("claude-haiku", &caps(None, Some(true))),
            None
        );
    }

    #[test]
    fn dual_flagged_model_is_rejected() {
        // The local runtime starts any text_embedding model with
        // `--embeddings`, so even a model that also claims `chat` cannot
        // compute logits — reject it.
        assert!(
            generation_unsupported_reason("dual", &caps(Some(true), Some(true))).is_some(),
            "a model flagged text_embedding must be rejected even if also flagged chat"
        );
    }

    #[test]
    fn unflagged_model_is_allowed() {
        // Manually-added chat models often have no capability flags set;
        // don't false-positive them.
        assert_eq!(
            generation_unsupported_reason("custom-llm", &ModelCapabilities::default()),
            None
        );
    }

    #[test]
    fn text_embedding_false_is_allowed() {
        assert_eq!(
            generation_unsupported_reason("gen", &caps(Some(false), None)),
            None
        );
    }

    // ── embedding_unsupported_reason (the inverse guard) ──────────────

    #[test]
    fn embedder_is_allowed_as_embedding_model() {
        assert_eq!(
            embedding_unsupported_reason("nomic-embed", &caps(Some(true), None)),
            None
        );
    }

    #[test]
    fn chat_model_is_rejected_as_embedding_model() {
        let reason = embedding_unsupported_reason("claude-haiku", &caps(None, Some(true)))
            .expect("a chat-only model must be rejected as an embedding model");
        assert!(reason.contains("claude-haiku"), "reason names the model: {reason}");
        assert!(reason.contains("text_embedding"), "reason mentions the capability: {reason}");
    }

    #[test]
    fn unflagged_model_is_rejected_as_embedding_model() {
        // The inverse guard is strict (positive capability required),
        // matching dispatch.rs — unlike the generation guard which
        // allows unflagged models.
        assert!(
            embedding_unsupported_reason("custom", &ModelCapabilities::default()).is_some(),
            "an unflagged model must be rejected as an embedding model"
        );
    }

    #[test]
    fn text_embedding_false_is_rejected_as_embedding_model() {
        assert!(
            embedding_unsupported_reason("gen", &caps(Some(false), None)).is_some(),
            "text_embedding=false must be rejected as an embedding model"
        );
    }
}
