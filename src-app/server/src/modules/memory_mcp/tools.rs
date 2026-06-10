//! Static tool descriptors emitted by `tools/list`.

use serde_json::{Value, json};

pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "remember",
                "description": "Persist a durable, non-obvious fact about the user to long-term memory. Use for explicit 'remember that …' requests. Pick the narrowest scope that fits.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The fact to remember, one sentence, third-person about the user."
                        },
                        "scope": {
                            "type": "string",
                            "enum": ["user", "project", "conversation"],
                            "default": "conversation",
                            "description": "Where this fact applies: 'conversation' = only this thread; 'project' = this project's work; 'user' = always true about the person across everything."
                        },
                        "kind": {
                            "type": "string",
                            "enum": ["preference", "fact", "goal", "relationship", "other"],
                            "default": "fact"
                        },
                        "importance": {
                            "type": "integer",
                            "minimum": 0,
                            "maximum": 100,
                            "default": 50
                        }
                    },
                    "required": ["content"]
                }
            },
            {
                "name": "recall",
                "description": "Search the user's memories by relevance to a query — hybrid semantic + full-text when an embedding model is configured, otherwise keyword/full-text only. Returns up to top_k matches; pick terms and iterate.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "top_k": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 50,
                            "default": 8
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "forget",
                "description": "Delete a single memory by id. The caller must own the memory; cross-user deletion is rejected.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "memory_id": { "type": "string", "format": "uuid" }
                    },
                    "required": ["memory_id"]
                }
            }
        ]
    })
}
