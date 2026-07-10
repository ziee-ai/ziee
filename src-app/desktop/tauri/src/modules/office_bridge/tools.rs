//! Static tool descriptors emitted by `tools/list`.
//!
//! The `office` tool surface: one native tool (`list_open_documents`, served by
//! the COM/osascript daemon via `platform::active()`) plus six **pane-mediated**
//! tools that route to a connected Office.js task pane over the bridge
//! (`bridge/broker.rs`). Five are typed/structured reads + gated edits
//! (`read_document`, `get_selection`, `add_comment`, `set_track_changes`,
//! `get_tracked_changes`); the sixth, `run_office_js`, is the open-ended breadth
//! surface — the model writes an Office.js body the pane executes inside the
//! host's `{Word,Excel,PowerPoint}.run`, so "everything Office.js supports" is
//! reachable at ~one tool schema of context cost instead of one tool per API.
//! (The former native-only `edit_document`/`append_paragraph` op is removed —
//! `run_office_js` subsumes it via `body.insertParagraph`.)

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
                "name": "run_office_js",
                "description": "Run an Office.js script against one open Office document, identified by its `doc_full_name`. This is the general-purpose way to read or change the document: you write the body of an async function that receives the Office.js request `context` for the document's host app (Word, Excel, or PowerPoint — selected automatically from the target document), and it runs inside the host's `Word.run` / `Excel.run` / `PowerPoint.run`. You may `await context.sync()` and `return` a JSON-serializable value, which is returned to you. Example (Excel): `const s = context.workbook.worksheets.getActiveWorksheet(); const r = s.getRange('A1'); r.values = [['hello']]; await context.sync(); r.load('address'); await context.sync(); return r.address;`. Requires the document's task pane to be open. On a script error, a structured error (name, message, Office.js error code) is returned so you can correct and retry.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "doc_full_name": {
                            "type": "string",
                            "description": "The app-qualified full name of the target document (from list_open_documents)."
                        },
                        "script": {
                            "type": "string",
                            "description": "The Office.js script body to run. It receives `context` (the host's request context) in scope, may `await context.sync()`, and may `return` a JSON-serializable value."
                        }
                    },
                    "required": ["doc_full_name", "script"]
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

    /// TEST-2 — `tool_list()` advertises EXACTLY the seven `office` tools:
    /// `run_office_js` is present, the removed `edit_document` is absent.
    #[test]
    fn tool_list_contains_exactly_the_seven_tools() {
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
            "run_office_js",
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
        assert!(
            !names.contains(&"edit_document"),
            "removed `edit_document` must be absent (had {names:?})"
        );
        assert_eq!(names.len(), 7, "expected exactly 7 tools, got {names:?}");
    }

    /// TEST-1 — `run_office_js` advertises the expected schema: `doc_full_name`
    /// and `script` are both required string properties.
    #[test]
    fn run_office_js_schema_requires_doc_and_script() {
        let list = tool_list();
        let tool = list["tools"]
            .as_array()
            .expect("tools is an array")
            .iter()
            .find(|t| t["name"] == "run_office_js")
            .expect("run_office_js present");
        let schema = &tool["inputSchema"];
        assert_eq!(schema["properties"]["doc_full_name"]["type"], "string");
        assert_eq!(schema["properties"]["script"]["type"], "string");
        let required: Vec<&str> = schema["required"]
            .as_array()
            .expect("required is an array")
            .iter()
            .map(|v| v.as_str().expect("required entry is a string"))
            .collect();
        assert!(required.contains(&"doc_full_name"), "doc_full_name required");
        assert!(required.contains(&"script"), "script required");
    }
}
