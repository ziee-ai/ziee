// Chat extensions
//
// `file`, `project`, `mcp`, `assistant`, and `memory` extensions live
// in their owning modules at `modules/<x>/chat_extension/` — chat
// picks them up via linkme's CHAT_EXTENSIONS distributed slice
// without importing them. Only chat-internal cross-cutters remain
// here.
pub mod text; // Text content extension
pub mod title; // Title generation extension
