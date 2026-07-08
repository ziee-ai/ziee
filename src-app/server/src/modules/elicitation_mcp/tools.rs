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
                "description": "Ask the human user a question and PAUSE until they answer. Use this whenever you need a decision, a missing detail, or a choice that only the user can make — instead of guessing. The user's answer is returned as the tool result so you can continue. Prefer this over assuming.\n\n`schema` is a JSON Schema object; each entry in `properties` is ONE question, rendered as a decision card. Ask 1–4 questions in a single call — with 2+ questions the UI shows a Next/Back wizard and returns all answers together, so batch related questions instead of calling the tool repeatedly.\n\nFor a MULTIPLE-CHOICE question use `enum` with, index-aligned, `enumNames` (short option labels) and `enumDescriptions` (one line explaining each option's trade-off — always provide these so the user can choose well). Flag your suggested option with `x-ziee-recommended: \"<that enum value>\"` (rendered first, badged \"Recommended\"). The user always gets an \"Other\" free-text escape on choice questions unless you set `x-ziee-allow-other: false`. Optionally add index-aligned `enumPreviews` (a short monospace snippet per option) to help the user compare. For multi-select, use `type: \"array\"` with `items: { enum, enumNames, enumDescriptions }`.\n\nFor free-form input use `format` (email/uri/date/date-time/password) or `pattern` for validated text, `minimum`/`maximum` for numbers, `type: boolean` for yes/no. Give each property a `title` (the question text) and `description`, and list `required` fields. Keep it to the few questions you actually need.",
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

    /// The descriptor teaches the model the rich decision-UX conventions so it
    /// emits per-option descriptions, a recommended marker, and the Other-escape
    /// opt-out. Guards against a silent revert of the guidance text.
    #[test]
    fn description_documents_rich_conventions() {
        let list = tool_list();
        let desc = list["tools"][0]["description"]
            .as_str()
            .expect("description string");
        for needle in [
            "enumDescriptions",
            "enumNames",
            "enumPreviews",
            "x-ziee-recommended",
            "x-ziee-allow-other",
            "wizard",
        ] {
            assert!(
                desc.contains(needle),
                "ask_user description must mention `{needle}` so the model emits rich schemas"
            );
        }
    }
}
