// Chat extension registration using linkme
//
// Extensions self-register using the CHAT_EXTENSIONS distributed slice
// and are initialized here in order based on their metadata.order value

use std::sync::Arc;
use sqlx::PgPool;

use crate::core::config::Config;
use crate::modules::chat::core::extension::{CHAT_EXTENSIONS, ExtensionRegistry};

/// Register all discovered extensions in order
pub fn auto_register_extensions(pool: PgPool, config: Arc<Config>) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();

    // Collect and sort extensions by order
    let mut entries: Vec<_> = CHAT_EXTENSIONS.iter().collect();
    entries.sort_by_key(|e| e.order);

    // Register each extension in order
    for entry in entries {
        tracing::debug!(
            "Registering chat extension: {} (order: {})",
            entry.name,
            entry.order
        );
        let extension = (entry.factory)(pool.clone(), config.clone());
        registry.register(extension);
    }

    registry
}

#[cfg(test)]
mod tests {
    use super::CHAT_EXTENSIONS;

    /// Reproduce the production pipeline ordering exactly as
    /// `auto_register_extensions` does (sort the linkme-collected
    /// `CHAT_EXTENSIONS` slice by `order`) and read back the resulting
    /// sequence of extension names — i.e. the order `before_llm_call`
    /// hooks actually fire in.
    fn sorted_pipeline_names() -> Vec<&'static str> {
        let mut entries: Vec<_> = CHAT_EXTENSIONS.iter().collect();
        entries.sort_by_key(|e| e.order);
        entries.iter().map(|e| e.name).collect()
    }

    /// Cross-module ordering contract (audit all-6907cfab0cd8):
    /// summarization (order 24) must run BEFORE memory (order 25) in the
    /// real chat-extension pipeline, so a turn is condensed first and the
    /// memory extension then injects retrieved memories around the already
    /// summary-collapsed message stack — not the other way around.
    ///
    /// This reads the genuine linkme-registered + sorted order (the same
    /// `sort_by_key(order)` the production registrar uses), NOT a hardcoded
    /// `24 < 25` comparison: if either extension's `order` is changed such
    /// that memory would condense/retrieve before summarization collapses
    /// the history, the asserted position relation breaks.
    #[test]
    fn summarization_runs_before_memory_in_the_registered_pipeline() {
        let names = sorted_pipeline_names();

        let summ_pos = names.iter().position(|&n| n == "summarization");
        let mem_pos = names.iter().position(|&n| n == "memory");

        // Both extensions must actually be registered into the pipeline.
        let summ_pos = summ_pos.expect("summarization extension must be registered in CHAT_EXTENSIONS");
        let mem_pos = mem_pos.expect("memory extension must be registered in CHAT_EXTENSIONS");

        // The defining contract: summarization precedes memory in the
        // sorted pipeline (read from the real registry order).
        assert!(
            summ_pos < mem_pos,
            "summarization (pos {summ_pos}) must run before memory (pos {mem_pos}) in the chat-extension pipeline; \
             full order = {names:?}",
        );

        // And there is nothing wedged between them that would reorder the
        // condense-then-retrieve handoff (they are adjacent in the pipeline).
        assert_eq!(
            mem_pos,
            summ_pos + 1,
            "memory must immediately follow summarization (no extension interposed); order = {names:?}",
        );
    }

    /// Guard the underlying `order` values that produce the relation above,
    /// read from the actual registered entries (not literals in this test).
    #[test]
    fn summarization_order_is_strictly_below_memory_order() {
        let mut summ = None;
        let mut mem = None;
        for e in CHAT_EXTENSIONS.iter() {
            match e.name {
                "summarization" => summ = Some(e.order),
                "memory" => mem = Some(e.order),
                _ => {}
            }
        }
        let summ = summ.expect("summarization registered");
        let mem = mem.expect("memory registered");
        assert!(
            summ < mem,
            "summarization order ({summ}) must be < memory order ({mem})",
        );
    }
}
