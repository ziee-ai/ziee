//! `tools/list` descriptor for the tool_result MCP server.

use serde_json::{Value, json};

pub fn tool_list() -> Value {
    json!({
        "tools": [{
            "name": "get_tool_result",
            "description": "Retrieve the stored content of an earlier tool result in THIS conversation by its tool_use_id — including its structuredContent — WITHOUT re-running the tool. This is an exact, read-only read of stored history (deterministic; re-running a live tool would return different results). Use it to: (1) recover a result that was cleared/truncated to save context — the placeholder text gives you the tool_use_id; (2) read the full structured detail (e.g. the full abstracts of an earlier literature_search) before deciding which papers to fetch in full. Page large results with offset/max_chars.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_use_id": {
                        "type": "string",
                        "description": "The tool_use_id of the prior tool result to retrieve (shown in the cleared/truncated placeholder, or the id of the originating tool call)."
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Character offset to start from, for paging large results. Default 0."
                    },
                    "max_chars": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Maximum characters to return. Default 8000."
                    }
                },
                "required": ["tool_use_id"]
            }
        }]
    })
}
