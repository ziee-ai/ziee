// Project chat extension
//
// Injects per-project context (instructions + attached files) into every
// chat send for conversations that belong to a project. Sits at order 8
// — runs BEFORE the assistant extension (order 10) so the assistant
// block ends up at index 0 (older position) and the project block at
// index 1 (closer to the user message; stronger recency). Both apply —
// orthogonal layers per the locked Plan 5 §4 semantics.

mod project;
pub mod extension; // Auto-discovered by the chat extension registration system
