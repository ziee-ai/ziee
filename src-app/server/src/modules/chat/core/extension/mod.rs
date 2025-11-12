// Extension system for chat module
pub mod metadata;
pub mod registry;
pub mod request;

// Re-exports
pub use metadata::ExtensionMetadata;
pub use registry::{ChatExtension, ExtensionAction, ExtensionRegistry, StreamContext};
pub use request::SendMessageRequest;
