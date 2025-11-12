// Extension metadata structure

/// Metadata for chat extensions
/// This struct is used in extension.rs files to define extension properties
pub struct ExtensionMetadata {
    /// Extension name (for logging and debugging)
    pub name: &'static str,
    /// Execution order (lower numbers execute first)
    /// Recommended ranges:
    /// - 0-19: System/infrastructure extensions
    /// - 20-39: Authentication/authorization extensions
    /// - 40-59: Content modification extensions
    /// - 60-79: Analytics/logging extensions
    /// - 80-99: Post-processing extensions
    pub order: i32,
}
