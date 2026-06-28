// Project extension metadata structure
//
// Mirrors `modules/chat/core/extension/metadata.rs` — kept as a separate
// struct so the two extension systems can evolve independently if a future
// project-extension needs metadata fields chat doesn't (or vice-versa).

/// Metadata for project extensions.
///
/// Order ranges are advisory; project extensions are short-lived and
/// fan-out is small, but keep ranges aligned with the chat-extension
/// convention for cross-pattern familiarity.
///
/// Recommended ranges:
/// - 0-19: System/infrastructure extensions
/// - 20-39: Authentication/authorization extensions
/// - 40-59: Content extensions (knowledge kinds: files, urls, notes, etc.)
/// - 60-79: Analytics/logging extensions
/// - 80-99: Post-processing extensions
#[allow(dead_code)]
pub struct ProjectExtensionMetadata {
    /// Extension name (for logging and debugging).
    pub name: &'static str,
    /// Execution order (lower numbers register/run first).
    pub order: i32,
}
