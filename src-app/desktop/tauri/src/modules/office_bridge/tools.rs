//! Static tool descriptors emitted by `tools/list`.
//!
//! The `office` tool surface is TWO tools: `list_open_documents` (native discovery,
//! served by the COM/osascript daemon via `platform::active()`, needs no task pane)
//! and `run_office_js` (the open-ended pane surface — the model writes an Office.js
//! body the connected task pane executes inside the host's `{Word,Excel,PowerPoint}.run`,
//! so "everything Office.js supports" is reachable at ~one tool schema). The former
//! typed tools (`read_document` / `get_selection` / `add_comment` / `set_track_changes`
//! / `get_tracked_changes`) are removed — `run_office_js` subsumes all of them.
//!
//! `run_office_js` declares `mode: "read" | "write"`. `mode` is an APPROVAL hint
//! consumed only by the server's MCP approval loop (a `read` auto-runs; a `write`
//! prompts the user); the daemon and the pane ignore it — execution is identical
//! either way. There is no pane-side read-only enforcement; the model is trusted to
//! declare `mode` honestly (see the office-mode-gated-approval lifecycle decisions).

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
                "name": "run_office_js",
                "description": "Run an Office.js script against one open Office document, identified by its `doc_full_name` (from list_open_documents). This is the general-purpose way to read or change the document — it replaces separate read/comment/track-changes tools. You write the body of an async function that receives the Office.js request `context` for the document's host app (Word, Excel, or PowerPoint — selected automatically from the target document); it runs inside the host's `Word.run` / `Excel.run` / `PowerPoint.run`. You may `await context.sync()` and `return` a JSON-serializable value, which is returned to you. On a script error, a structured error (name, message, Office.js error code) is returned so you can correct and retry. Requires the document's task pane to be open.\n\nDeclare `mode`: use \"read\" ONLY when the script exclusively reads (no property assignments, no `insert*`/`delete`/`add`/`merge`, no `changeTrackingMode`, no `insertComment`); use \"write\" for ANY change to the document. A \"write\" asks the user for approval before running; a \"read\" runs immediately. When unsure, use \"write\".\n\nExamples — Read a cell (mode \"read\"): `const r = context.workbook.worksheets.getActiveWorksheet().getRange('A1'); r.load('values'); await context.sync(); return r.values;`. Write a cell (mode \"write\"): `const r = context.workbook.worksheets.getActiveWorksheet().getRange('A1'); r.values = [['hello']]; r.load('address'); await context.sync(); return r.address;`. Add a Word comment (mode \"write\"): `const res = context.document.body.search('Q3 revenue', {matchCase:false}); res.load('items'); await context.sync(); res.items[0].insertComment('cite the source'); await context.sync();`. Toggle Word tracked changes (mode \"write\"): `context.document.changeTrackingMode = Word.ChangeTrackingMode.trackAll; await context.sync();`.",
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
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["read", "write"],
                            "description": "\"read\" if the script only reads the document (no changes); \"write\" if it changes anything. A \"write\" requires user approval before it runs; a \"read\" runs without prompting. When unsure, use \"write\"."
                        }
                    },
                    "required": ["doc_full_name", "script", "mode"]
                }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-1 — `tool_list()` advertises EXACTLY the two surviving `office` tools;
    /// all five pruned typed tools are absent.
    #[test]
    fn tool_list_contains_exactly_the_two_tools() {
        let list = tool_list();
        let names: Vec<&str> = list["tools"]
            .as_array()
            .expect("tools is an array")
            .iter()
            .map(|t| t["name"].as_str().expect("tool name is a string"))
            .collect();
        for expected in ["list_open_documents", "run_office_js"] {
            assert!(
                names.contains(&expected),
                "tool_list missing `{expected}` (had {names:?})"
            );
        }
        for pruned in [
            "read_document",
            "get_selection",
            "add_comment",
            "set_track_changes",
            "get_tracked_changes",
        ] {
            assert!(
                !names.contains(&pruned),
                "pruned tool `{pruned}` must be absent (had {names:?})"
            );
        }
        assert_eq!(names.len(), 2, "expected exactly 2 tools, got {names:?}");
    }

    /// TEST-2 — `run_office_js` requires `doc_full_name` + `script` + `mode` (an enum
    /// `["read","write"]`), and its description carries the read/write approval
    /// guidance so the model is actually told how to set `mode` and that writes prompt.
    #[test]
    fn run_office_js_schema_has_mode_and_description_guidance() {
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
        assert_eq!(schema["properties"]["mode"]["type"], "string");
        let mode_enum: Vec<&str> = schema["properties"]["mode"]["enum"]
            .as_array()
            .expect("mode enum is an array")
            .iter()
            .map(|v| v.as_str().expect("enum entry is a string"))
            .collect();
        assert_eq!(mode_enum, vec!["read", "write"], "mode enum is read|write");
        let required: Vec<&str> = schema["required"]
            .as_array()
            .expect("required is an array")
            .iter()
            .map(|v| v.as_str().expect("required entry is a string"))
            .collect();
        assert!(required.contains(&"doc_full_name"), "doc_full_name required");
        assert!(required.contains(&"script"), "script required");
        assert!(required.contains(&"mode"), "mode required");

        // The description must actually teach the model the mode contract.
        let desc = tool["description"].as_str().expect("description is a string");
        assert!(desc.contains("mode"), "description mentions mode");
        assert!(desc.contains("read") && desc.contains("write"), "read/write guidance");
        assert!(desc.contains("approval"), "tells the model a write needs approval");
    }
}
