//! Static MCP tool descriptors for the citations server. Batch-first: the
//! resolve/add/verify tools take an `items` array of `CitationInput` and return
//! a per-item report, so a whole manuscript reference list is one tool call.

use serde_json::{Value, json};

/// A reusable JSON-Schema fragment for one `CitationInput` (see models.rs).
fn citation_input_schema() -> Value {
    json!({
        "type": "object",
        "description": "Send the minimum you know. NEVER required to supply a DOI — the server resolves and cross-checks. At least one of id/title/csl/raw.",
        "properties": {
            "id": { "type": "string", "description": "Raw identifier: DOI, PMID, PMCID, or arXiv id (auto-detected). May be wrong/fabricated — it is checked." },
            "kind": { "type": "string", "enum": ["doi", "pmid", "pmcid", "arxiv"], "description": "Optional explicit id kind." },
            "title": { "type": "string", "description": "Free-text title when there is no/uncertain id; the server title-searches to find the real record." },
            "authors": { "type": "array", "items": { "type": "string" } },
            "year": { "type": "integer" },
            "journal": { "type": "string" },
            "csl": { "type": "object", "description": "A full CSL-JSON item (e.g. from a prior literature_search result)." },
            "raw": { "type": "string", "description": "A raw reference string to parse/search." }
        }
    })
}

/// The tools/list payload.
pub fn tool_list() -> Value {
    let items = json!({
        "type": "array",
        "maxItems": 100,
        "items": citation_input_schema()
    });

    json!({
        "tools": [
            {
                "name": "lookup_citations",
                "description": "Resolve + verify a batch of references to canonical CSL-JSON WITHOUT saving them — resolves real records (Crossref/PubMed/arXiv/doi.org). Returns per-item resolved metadata + verification status (verified/mismatch/not_found/unverified).",
                "inputSchema": {
                    "type": "object",
                    "properties": { "items": items.clone() },
                    "required": ["items"]
                }
            },
            {
                "name": "add_citations",
                "description": "Resolve + verify + de-duplicate + add a batch of references to the user's bibliography (optionally also into a project's reference list). Returns a per-item report: inserted | linked_existing | possible_duplicate | failed, plus verification status. Never invents a citation — fabricated DOIs come back not_found.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "format": "uuid", "description": "Optional: also link the added entries into this project's reference list." },
                        "items": items.clone()
                    },
                    "required": ["items"]
                }
            },
            {
                "name": "verify_citations",
                "description": "Check whether each reference resolves to a REAL record (the fabrication checker). Returns per-item verified/mismatch/not_found/unverified with the resolved DOI/PMID when found.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "items": items },
                    "required": ["items"]
                }
            },
            {
                "name": "list_citations",
                "description": "List the user's bibliography entries (or, with project_id, a single project's reference list) with their citation keys and verification status.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "format": "uuid", "description": "Optional: restrict to this project's reference list." }
                    }
                }
            },
            {
                "name": "format_citations",
                "description": "Render a reference list in a chosen CSL style + format. format is one of csljson | bibtex | ris | text; style is a bundled CSL style name (e.g. apa, vancouver, nature) used when format=text. Supply EITHER inline `items` (raw CSL-JSON, formatted directly — nothing is saved) OR saved `ids`/`project_id`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "items": { "type": "array", "items": { "type": "object" }, "description": "Inline CSL-JSON references to format directly (e.g. from a workflow's own records). When given, `ids`/`project_id` are ignored and NOTHING is written to the library." },
                        "project_id": { "type": "string", "format": "uuid" },
                        "ids": { "type": "array", "items": { "type": "string", "format": "uuid" } },
                        "style": { "type": "string", "description": "CSL style name (for format=text). Default apa." },
                        "format": { "type": "string", "enum": ["csljson", "bibtex", "ris", "text"], "description": "Output format. Default text." }
                    }
                }
            },
            {
                "name": "remove_citations",
                "description": "Remove entries from the library entirely (ids), or — with project_id — only unlink them from that project's reference list (the entries stay in the library).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "format": "uuid", "description": "If set, only unlink from this project (do not delete)." },
                        "ids": { "type": "array", "items": { "type": "string", "format": "uuid" } }
                    },
                    "required": ["ids"]
                }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_has_six_batch_tools() {
        let v = tool_list();
        let tools = v["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 6);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for expected in [
            "lookup_citations",
            "add_citations",
            "verify_citations",
            "list_citations",
            "format_citations",
            "remove_citations",
        ] {
            assert!(names.contains(&expected), "missing tool {expected}");
        }
    }

    #[test]
    fn batch_tools_cap_items_at_100() {
        let v = tool_list();
        let add = v["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "add_citations")
            .unwrap();
        assert_eq!(add["inputSchema"]["properties"]["items"]["maxItems"], 100);
    }
}
