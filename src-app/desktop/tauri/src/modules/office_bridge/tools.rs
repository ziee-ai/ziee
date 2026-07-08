//! Static tool descriptors emitted by `tools/list`.
//!
//! The full `office` tool surface (ITEM-9). Two of these are served *now* by the
//! native COM daemon via `platform::active()` — `list_open_documents` and
//! `edit_document`'s `append_paragraph` op. The remaining five are
//! **pane-mediated** (they need an open Office.js task pane over the bridge,
//! which lands in a later item); until that RPC is wired, `dispatch_tool`
//! answers them with an honest, typed capability error rather than pretending
//! to act.

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
            },
            {
                "name": "read_document",
                "description": "Read the text content of one open Office document, identified by its `doc_full_name` (as returned by list_open_documents). Requires the document's task pane to be open. Returns the document body as plain text.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "doc_full_name": {
                            "type": "string",
                            "description": "The app-qualified full name of the target document (from list_open_documents)."
                        }
                    },
                    "required": ["doc_full_name"]
                }
            },
            {
                "name": "edit_document",
                "description": "Apply an edit to one open Office document, identified by its `doc_full_name`. The `append_paragraph` operation adds a paragraph of text to the end of the document body and saves it; this operation works natively today. Returns whether the edit landed plus a short read-back of the appended text.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "doc_full_name": {
                            "type": "string",
                            "description": "The app-qualified full name of the target document (from list_open_documents)."
                        },
                        "op": {
                            "type": "string",
                            "enum": ["append_paragraph"],
                            "description": "The edit operation to apply. Currently only `append_paragraph`."
                        },
                        "text": {
                            "type": "string",
                            "description": "The paragraph text to append (for op = append_paragraph)."
                        }
                    },
                    "required": ["doc_full_name", "op", "text"]
                }
            },
            {
                "name": "add_comment",
                "description": "Attach a review comment to a span of text in one open Office document. Anchors on the first occurrence of `anchor_text`. Requires the document's task pane to be open. Not supported on PowerPoint.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "doc_full_name": {
                            "type": "string",
                            "description": "The app-qualified full name of the target document (from list_open_documents)."
                        },
                        "anchor_text": {
                            "type": "string",
                            "description": "The existing text to anchor the comment on (first match)."
                        },
                        "text": {
                            "type": "string",
                            "description": "The comment body."
                        }
                    },
                    "required": ["doc_full_name", "anchor_text", "text"]
                }
            },
            {
                "name": "set_track_changes",
                "description": "Turn tracked changes (revision marking) on or off for one open Office document. Requires the document's task pane to be open. Not supported on PowerPoint.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "doc_full_name": {
                            "type": "string",
                            "description": "The app-qualified full name of the target document (from list_open_documents)."
                        },
                        "enabled": {
                            "type": "boolean",
                            "description": "True to enable tracked changes, false to disable."
                        }
                    },
                    "required": ["doc_full_name", "enabled"]
                }
            },
            {
                "name": "get_tracked_changes",
                "description": "List the tracked changes (insertions, deletions, revisions) currently recorded in one open Office document. Requires the document's task pane to be open.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "doc_full_name": {
                            "type": "string",
                            "description": "The app-qualified full name of the target document (from list_open_documents)."
                        }
                    },
                    "required": ["doc_full_name"]
                }
            },
            {
                "name": "get_selection",
                "description": "Return the text the user currently has selected in one open Office document. Requires the document's task pane to be open.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "doc_full_name": {
                            "type": "string",
                            "description": "The app-qualified full name of the target document (from list_open_documents)."
                        }
                    },
                    "required": ["doc_full_name"]
                }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-12 (unit slice) — `tool_list()` advertises all seven `office` tools.
    #[test]
    fn tool_list_contains_all_seven_tools() {
        let list = tool_list();
        let names: Vec<&str> = list["tools"]
            .as_array()
            .expect("tools is an array")
            .iter()
            .map(|t| t["name"].as_str().expect("tool name is a string"))
            .collect();
        for expected in [
            "list_open_documents",
            "read_document",
            "edit_document",
            "add_comment",
            "set_track_changes",
            "get_tracked_changes",
            "get_selection",
        ] {
            assert!(
                names.contains(&expected),
                "tool_list missing `{expected}` (had {names:?})"
            );
        }
        assert_eq!(names.len(), 7, "expected exactly 7 tools, got {names:?}");
    }
}
