//! Static tool descriptor emitted by `tools/list`.

use serde_json::{Value, json};

/// The single tool: `ask_user`. The `schema` argument is a JSON Schema
/// (object) describing the field(s) to collect; the chat frontend renders
/// it as a form (enum → choice, `format`/`pattern` → validated input,
/// booleans, numbers, required, defaults).
pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "ask_user",
                "description": "Ask the human user a question and PAUSE until they answer. Use this whenever you need a decision, a missing detail, or a choice that only the user can make — instead of guessing. The user's answer is returned as the tool result so you can continue. Prefer this over assuming. `schema` is a JSON Schema object describing the field(s) to collect: use `enum` (optionally with `enumNames`) for multiple-choice, `format` (email/uri/date/date-time) or `pattern` for validated input, `minimum`/`maximum` for numbers, `type: boolean` for yes/no, and list `required` fields. Keep it to the few fields you actually need.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "The question/prompt shown to the user above the form."
                        },
                        "schema": {
                            "type": "object",
                            "description": "JSON Schema for the requested input. Typically { type: 'object', properties: {...}, required: [...] }. Use enum for choices and format/pattern for validated values."
                        }
                    },
                    "required": ["message", "schema"]
                }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_ask_user_tool() {
        let list = tool_list();
        let tools = list["tools"].as_array().expect("tools array");
        assert_eq!(tools.len(), 1, "exactly one tool");
        let t = &tools[0];
        assert_eq!(t["name"], "ask_user");
        let required = t["inputSchema"]["required"]
            .as_array()
            .expect("required array");
        assert!(required.iter().any(|v| v == "message"));
        assert!(required.iter().any(|v| v == "schema"));
        // message + schema are both object properties.
        assert_eq!(t["inputSchema"]["properties"]["message"]["type"], "string");
        assert_eq!(t["inputSchema"]["properties"]["schema"]["type"], "object");
    }
}
