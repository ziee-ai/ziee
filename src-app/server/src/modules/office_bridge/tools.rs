//! Static tool descriptors emitted by `tools/list`.
//!
//! First increment: one read-only descriptor. Read/edit/comment/track-changes
//! tools + the actual dispatch land in ITEM-9.

use serde_json::{Value, json};

pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "list_open_documents",
                "description": "List the Microsoft Office documents (Word, Excel, PowerPoint) currently open on the user's desktop, with each document's name, full path, host application, and saved state. Returns an empty list when no Office documents are open or the host has no Office bridge available.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        ]
    })
}
