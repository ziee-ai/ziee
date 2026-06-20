//! citations DTOs — the bibliography entry (CSL-JSON + projected scalars), the
//! flexible per-item input the model/UI sends, and the per-item batch report.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Verification outcome for a stored or checked reference.
///
/// `not_found` is reserved for a **supplied identifier that fails to resolve**
/// (the fabricated-DOI case). An entry with no identifier at all rests at
/// `unverified` — absence of an id is NOT a red flag (books, theses, grey
/// literature, datasets, in-press all legitimately lack a DOI/PMID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unverified,
    Verified,
    Mismatch,
    NotFound,
}

impl VerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            VerificationStatus::Unverified => "unverified",
            VerificationStatus::Verified => "verified",
            VerificationStatus::Mismatch => "mismatch",
            VerificationStatus::NotFound => "not_found",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "verified" => VerificationStatus::Verified,
            "mismatch" => VerificationStatus::Mismatch,
            "not_found" => VerificationStatus::NotFound,
            _ => VerificationStatus::Unverified,
        }
    }
}

/// What happened to an item on an add/import path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DedupOutcome {
    /// New row created.
    Inserted,
    /// Matched an existing library row by id/exact-fingerprint → linked to it
    /// (the existing entry is reused; optionally attached to the project).
    LinkedExisting,
    /// A fuzzy near-match was found; NOT auto-merged — surfaced for user review.
    PossibleDuplicate,
    /// Could not be processed (parse/resolve error); see `reason`.
    Failed,
}

/// A bibliography library entry as returned to the API/UI. `csl_json` is the
/// canonical record; the scalar fields are a projection of it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BibliographyEntry {
    pub id: Uuid,
    pub csl_json: Value,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub arxiv_id: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub citation_key: String,
    pub verification_status: VerificationStatus,
    pub verified_at: Option<DateTime<Utc>>,
    pub source: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The flexible per-item input the LLM (or UI/REST) sends. **At least one of**
/// `id` / `title` / `csl` / `raw` must be present; the model is NEVER required
/// to supply a DOI (the field it hallucinates most) — the server resolves +
/// cross-checks whatever it's given.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct CitationInput {
    /// A raw identifier (DOI / PMID / PMCID / arXiv); kind auto-detected by
    /// pattern, or set `kind` to disambiguate. May be wrong/fabricated.
    pub id: Option<String>,
    /// Optional explicit identifier kind: "doi" | "pmid" | "pmcid" | "arxiv".
    pub kind: Option<String>,
    /// A free-text reference with no/uncertain identifier — the server
    /// title-searches Crossref/PubMed to find the real record.
    pub title: Option<String>,
    pub authors: Option<Vec<String>>,
    pub year: Option<i32>,
    pub journal: Option<String>,
    /// A full CSL-JSON item (e.g. piped from a prior literature_search result).
    pub csl: Option<Value>,
    /// A raw reference string to parse for an identifier or title-search.
    pub raw: Option<String>,
}

/// One line of a batch (lookup/add/verify) report — the structured form behind
/// the import result view (`added · merged · already present · possible
/// duplicate · not found · mismatch · unverified · failed`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CitationItemResult {
    /// Echo of the input identifier/title so the UI can line results up.
    pub input: String,
    /// The library entry this resolved to (when persisted).
    pub entry_id: Option<Uuid>,
    pub citation_key: Option<String>,
    /// Present on add/import paths (None for pure verify/lookup).
    pub dedup_outcome: Option<DedupOutcome>,
    pub verification_status: VerificationStatus,
    /// For a `possible_duplicate` outcome, the existing entry to review against.
    pub possible_duplicate_of: Option<Uuid>,
    /// For `mismatch`, the fields that disagreed with the resolved record.
    pub mismatch_fields: Option<Vec<String>>,
    /// For `failed`/`not_found`, a short human reason.
    pub reason: Option<String>,
}

/// Max items accepted in one batch call (over-cap is reported, not truncated).
pub const MAX_BATCH_ITEMS: usize = 100;

// ─────────────────────────── REST DTOs ───────────────────────────

/// `?project_id=` filter for listing.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ListCitationsQuery {
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListCitationsResponse {
    pub entries: Vec<BibliographyEntry>,
}

/// Import / add by identifier or CSL-JSON (the REST analogue of `add_citations`).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportCitationsRequest {
    pub items: Vec<CitationInput>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
}

/// Verify a reference list without persisting (the REST analogue of `verify_citations`).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VerifyCitationsRequest {
    pub items: Vec<CitationInput>,
}

/// The per-item batch report returned by import / verify.
#[derive(Debug, Serialize, JsonSchema)]
pub struct BatchReport {
    pub results: Vec<CitationItemResult>,
}

/// Attach existing library entries into a project's reference list.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AttachCitationsRequest {
    pub entry_ids: Vec<Uuid>,
}

/// Generic mutation acknowledgement.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MutationResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ExportQuery {
    pub project_id: Option<Uuid>,
    /// csljson | bibtex | ris | text (default text)
    pub format: Option<String>,
    /// CSL style name for `text` (default: pandoc's built-in).
    pub style: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExportResponse {
    pub format: String,
    pub output: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StylesResponse {
    pub styles: Vec<String>,
}
