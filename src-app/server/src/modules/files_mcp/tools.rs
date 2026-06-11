//! Static tool descriptors emitted by `tools/list` — exactly three read-only
//! tools (LS / Read / Grep, adapted to a fixed id-addressed file set).

use serde_json::{Value, json};

pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "list_files",
                "description": "List the files available in this conversation (project knowledge files + attachments). Returns id, name, type, whether it has readable text, size and page count. Address files by `id`. A cheap manifest is already injected each turn; call this to refresh.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "read_file",
                "description": "Read a file's extracted text by `id` (preferred); `name` also works but only when it uniquely identifies one file. Slice large files with `offset`/`limit` — these are LINES for text/code files and PAGES for PDF/office documents. Images are returned for vision; binary/no-text files return a short note. Never reads files outside this conversation.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid", "description": "File id from list_files (preferred)." },
                        "name": { "type": "string", "description": "Filename — accepted only when it resolves to exactly one file." },
                        "offset": { "type": "integer", "minimum": 0, "description": "0-based start offset — counts lines for text/code files, pages for PDF/office documents." },
                        "limit": { "type": "integer", "minimum": 1, "description": "Max items to return — lines for text/code, pages for documents." }
                    }
                }
            },
            {
                "name": "grep_files",
                "description": "Lexical (regex) search over the extracted text of the available files. Optionally restrict to one file with `id`. Returns matching lines with file/page references. This is keyword search, not semantic — pick terms and iterate.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Regular expression." },
                        "id": { "type": "string", "format": "uuid", "description": "Optional: restrict to this file." },
                        "ignore_case": { "type": "boolean", "default": true }
                    },
                    "required": ["pattern"]
                }
            }
        ]
    })
}
