//! Static tool descriptors emitted by `tools/list` — 9 tools: 4 read-only
//! (list_files / read_file / grep_files / semantic_search, adapted to a fixed
//! id-addressed file set) + 5 write (create_file / edit_file / edit_file_lines /
//! rewrite_file / convert_document).

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
            },
            {
                "name": "semantic_search",
                "description": "Semantic search over the available files: finds passages by MEANING (vector similarity blended with keyword relevance), so it matches conceptually even when the wording differs. Optionally restrict to one file with `id`. Returns the most relevant passages with file/page and character-span references. Complements grep_files (exact regex) and read_file.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Natural-language description of what you're looking for." },
                        "top_k": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Max passages to return (defaults to the deployment setting)." },
                        "id": { "type": "string", "format": "uuid", "description": "Optional: restrict the search to this one file." }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "create_file",
                "description": "Create a new TEXT file (markdown, code, csv, json, …) with the given content. Returns its id and a resource_link. Edit it later with edit_file / rewrite_file. Use this to author a document the user can view and that you can revise across turns.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string", "description": "Name including extension, e.g. report.md or analysis.py." },
                        "content": { "type": "string", "description": "Full file contents." }
                    },
                    "required": ["filename", "content"]
                }
            },
            {
                "name": "edit_file",
                "description": "Edit a TEXT file by replacing the UNIQUE occurrence of `old_str` with `new_str` (a new version is appended; prior versions are kept and restorable). `old_str` must match exactly once — include enough surrounding context to be unique, or the call is rejected. Address the file by `id` (preferred) or unambiguous `name`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid", "description": "File id (preferred)." },
                        "name": { "type": "string", "description": "Filename — only when it resolves to exactly one file." },
                        "old_str": { "type": "string", "description": "The exact text to replace (must occur exactly once)." },
                        "new_str": { "type": "string", "description": "Replacement text." }
                    },
                    "required": ["old_str", "new_str"]
                }
            },
            {
                "name": "edit_file_lines",
                "description": "Edit a TEXT file by replacing the 1-indexed inclusive line range [start_line, end_line] with `new_content` (appends a new version). Set start_line = (line count + 1) to append. Address by `id` (preferred) or unambiguous `name`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "name": { "type": "string" },
                        "start_line": { "type": "integer", "minimum": 1 },
                        "end_line": { "type": "integer", "minimum": 0 },
                        "new_content": { "type": "string" }
                    },
                    "required": ["start_line", "end_line", "new_content"]
                }
            },
            {
                "name": "rewrite_file",
                "description": "Replace a TEXT file's ENTIRE contents with `content` (appends a new version). Use for large rewrites; prefer edit_file for targeted changes. Address by `id` (preferred) or unambiguous `name`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "name": { "type": "string" },
                        "content": { "type": "string", "description": "New full file contents." }
                    },
                    "required": ["content"]
                }
            },
            {
                "name": "convert_document",
                "description": "Convert Markdown to a PDF and save it to your files (returns the saved file). Use to turn a report or synthesis into a downloadable PDF. Rendered server-side with a UTF-8/scientific-symbol-safe engine.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "markdown": { "type": "string", "description": "The Markdown source to render to PDF." },
                        "filename": { "type": "string", "description": "Output filename (a '.pdf' extension is ensured). Default 'document.pdf'." }
                    },
                    "required": ["markdown"]
                }
            }
        ]
    })
}
