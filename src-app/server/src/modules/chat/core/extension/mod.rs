// Extension system for chat module
pub mod metadata;
pub mod registry;
pub mod request;

// Re-exports
pub use metadata::ExtensionMetadata;
pub use registry::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionAction, ExtensionEntry, ExtensionRegistry,
    StreamContext,
};
pub use request::SendMessageRequest;
