// Extension system for chat module
pub mod metadata;
pub mod registry;
pub mod request;

// Re-exports
pub use metadata::ExtensionMetadata;
pub use registry::{
    BeforeLlmAction, CHAT_EXTENSIONS, ChatExtension, ExtensionAction, ExtensionEntry,
    ExtensionRegistry, StreamContext, runtime_chat_extensions,
};
// `register_chat_extension` is exposed to the desktop crate via the `ziee`
// facade (lib.rs) directly from `registry` — nothing inside this crate calls it.
pub use request::SendMessageRequest;
