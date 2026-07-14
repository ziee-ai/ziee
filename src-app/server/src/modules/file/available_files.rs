//! Shared resolver for a conversation's *effective* file set.
//!
//! A conversation can "see" three kinds of files:
//!   - **project files** — the knowledge files of the project the conversation
//!     belongs to (M:N via `project_files`), if any;
//!   - **conversation attachments** — files referenced from `file_attachment`/
//!     `image` content blocks across the conversation's messages (the `file_id`
//!     lives in the `message_contents.content` JSONB, no FK);
//!   - **model-authored (generated) files** — files the MODEL produced in this
//!     conversation (`files_mcp`'s `create_file`/`convert_document`, or an MCP
//!     tool-result artifact ingested by `mcp::resource_link::persist_links`).
//!     These never get a `file_attachment` row, so they are resolved by
//!     provenance instead: `file_versions.source_message_id` → the
//!     conversation's branch messages, narrowed to `files.created_by IN
//!     ('mcp','llm')` (migration 34's model-authored vocabulary). WITHOUT this
//!     source the tools hand the model a file id (`"Created 'x' (id …)"`) and
//!     then reject that very id as "no longer available" — the read/write
//!     asymmetry that motivated this source (writes resolve by owner, reads by
//!     this manifest). Same definition as the derived list behind
//!     `deliverables.rs` — shared via `model_authored_file_ids`.
//!
//! This module is the source of truth for the built-in `files` MCP server's
//! manifest + read tools — "what files exist for this conversation". The code
//! sandbox mount (Track C) does NOT consume this resolver; it runs its own
//! active-branch query (`code_sandbox/repository.rs::get_conversation_files`).
//! The two paths agree on project files but DIVERGE on attachment branch-scope
//! intentionally: this resolver spans ALL branches (a file referenced from any
//! message is readable via the tools), while the sandbox mounts only the
//! conversation's active branch. Uniqueness is computed HERE, at point-of-use —
//! never by renaming stored files:
//!   - dedup by `file_id` (a file that's both a project file and an attachment,
//!     or referenced from multiple messages, appears once with a merged source
//!     set);
//!   - dedup by content `(user_id, checksum)` (an accidental re-upload, or the
//!     same bytes as both a project file and an attachment with different ids,
//!     collapses to one entry; the canonical entry is the first in resolution
//!     order — project files first, then attachments, each ordered by upload
//!     time — with the other filenames preserved as `aka` aliases).
//!
//! Every file is re-validated for ownership via `get_by_id_and_user`, so a
//! dangling JSONB reference (deleted file) or a foreign file is silently
//! dropped — the same "no longer available" path the read tools rely on.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::file::models::File;

/// Coarse LLM-facing file category (see the four extractability categories).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    /// Plain text / code / config / csv / md / html / json / yaml / ipynb …
    Text,
    /// PDF / office doc with extractable per-page text.
    Document,
    /// Raster image — vision, never text.
    Image,
    /// No extractable text and not an image (archive, audio/video, pptx, …).
    Binary,
}

/// Where a file is reachable from for this conversation. A file can be reachable
/// from MORE THAN ONE of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSource {
    Project,
    Attachment,
    /// Authored by the MODEL in this conversation (`created_by IN ('mcp','llm')`),
    /// reachable via `file_versions.source_message_id` provenance rather than an
    /// attachment content block.
    Generated,
}

/// One row of the conversation's manifest / mount set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableFile {
    pub id: Uuid,
    pub name: String,
    /// Other filenames for the SAME content (content-dedup aliases). Empty when
    /// this file's content is unique in the set.
    pub aka: Vec<String>,
    pub mime_type: Option<String>,
    #[serde(rename = "type")]
    pub file_type: FileType,
    /// Whether the file has LLM-readable extracted text.
    pub text: bool,
    pub size: i64,
    /// Number of extracted text pages. Plain text extracts as a single page
    /// (`pages == 1`); only files with no extractable text are `0`.
    pub pages: i32,
    /// All the ways this file is reachable (project and/or attachment).
    pub source: Vec<FileSource>,
    pub uploaded_at: DateTime<Utc>,
    /// Ids of the OTHER files this entry absorbed during content-dedup (same
    /// checksum). The model may legitimately hold one of these (a tool result
    /// handed it that id before the duplicate collapsed), so `resolve_target`
    /// accepts them as aliases for `id` — exactly as `aka` does for `name`.
    /// Internal: never surfaced, so the model always sees ONE canonical id.
    #[serde(skip)]
    pub aka_ids: Vec<Uuid>,
    /// sha256 (when known) — the content-dedup key. Not surfaced to the model.
    #[serde(skip)]
    pub checksum: Option<String>,
    /// Storage key for the HEAD version's bytes (= file_id for an un-versioned
    /// file, else the head version's blob). Reads must load by this, not `id`,
    /// so a versioned file serves its latest content. Internal.
    #[serde(skip)]
    pub blob_version_id: Uuid,
}

impl AvailableFile {
    fn from_file(f: &File, source: FileSource) -> Self {
        let mime = f.mime_type.as_deref().unwrap_or("application/octet-stream");
        let is_image = mime.starts_with("image/");
        let has_text = f.text_page_count > 0 || is_text_like(mime);
        let file_type = if is_image {
            FileType::Image
        } else if !has_text {
            FileType::Binary
        } else if is_doc_like(mime) {
            FileType::Document
        } else {
            FileType::Text
        };
        AvailableFile {
            id: f.id,
            name: f.filename.clone(),
            aka: Vec::new(),
            aka_ids: Vec::new(),
            mime_type: f.mime_type.clone(),
            file_type,
            text: has_text && !is_image,
            size: f.file_size,
            pages: f.text_page_count,
            source: vec![source],
            uploaded_at: f.created_at,
            checksum: f.checksum.clone(),
            blob_version_id: f.blob_version_id,
        }
    }

    fn add_source(&mut self, source: FileSource) {
        if !self.source.contains(&source) {
            self.source.push(source);
        }
    }
}

/// True for mimes the chat provider-routing inlines verbatim as text, AND any
/// text the upload pipeline's `looks_like_text` fallback extracted (those land
/// with `text_page_count > 0`, handled by the caller).
pub fn is_text_like(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/yaml"
                | "application/x-ndjson"
        )
}

/// PDF / office document mimes — paginated, fidelity-bearing.
pub fn is_doc_like(mime: &str) -> bool {
    mime == "application/pdf"
        || mime.starts_with("application/vnd.openxmlformats-officedocument")
        || mime.starts_with("application/vnd.oasis.opendocument")
        || matches!(mime, "application/msword" | "application/vnd.ms-excel")
}

const UUID_RE: &str =
    r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$";

/// Resolve whether the active model can use tools, from the chat
/// `StreamContext.metadata`. Source of truth (in order): the per-model persisted
/// `llm_models.capabilities.tools`, then the curated `model_registry` catalog
/// (keyed by `provider_type` + model **name**, NOT the DB uuid). `None`/absent
/// ⇒ treated as NOT tool-capable (conservative: fall back to inlining).
pub async fn model_supports_tools(
    metadata: &std::collections::HashMap<String, serde_json::Value>,
) -> bool {
    // 0. Memoized answer — seeded each LLM iteration by `ensure_model_tools_capable`
    //    from the earliest `before_llm_call` (which holds `&mut metadata`). The
    //    metadata map is rebuilt per tool-loop iteration, so the memo is re-seeded
    //    each iteration; the recomputed value is identical (model_id /
    //    provider_type / model_name don't change mid-turn). When present we skip
    //    the DB + catalog lookups for the rest of this iteration's before-calls.
    if let Some(b) = metadata
        .get("model_tools_capable")
        .and_then(|v| v.as_bool().or_else(|| v.as_str().and_then(parse_bool_ish)))
    {
        return b;
    }
    // 1. Per-model persisted capability (set for local models at validation, for
    //    cloud models at create time).
    if let Some(model_id) = metadata
        .get("model_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        match Repos.llm_model.get_by_id(model_id).await {
            Ok(Some(model)) => {
                if let Some(tools) = model.capabilities.tools {
                    return tools;
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(
                "model lookup failed during tool-capability resolution: {e}"
            ),
        }
    }
    // 2. Curated catalog fallback — lookup by model NAME.
    if let (Some(ptype), Some(mname)) = (
        metadata.get("provider_type").and_then(|v| v.as_str()),
        metadata.get("model_name").and_then(|v| v.as_str()),
    ) {
        if let Some(caps) = ai_providers::model_registry::lookup(ptype, mname) {
            if let Some(t) = caps.supports_tool_use {
                return t;
            }
        }
    }
    false
}

/// Parse a bool-ish string ("true"/"false", case-insensitive) into a bool.
/// Used so the memoized `model_tools_capable` metadata value round-trips
/// whether stored as a JSON bool or a JSON string.
fn parse_bool_ish(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Compute the model's tool-capability once and memoize it into `metadata`
/// under `model_tools_capable`, returning the result. Idempotent: if a prior
/// `before_llm_call` already seeded the key, this re-uses it (and
/// `model_supports_tools` short-circuits on it too). Call this at the TOP of
/// each chat extension's `before_llm_call` so whichever extension runs first
/// pays the lookup cost and the rest read the cached boolean.
pub async fn ensure_model_tools_capable(
    metadata: &mut std::collections::HashMap<String, serde_json::Value>,
) -> bool {
    if let Some(b) = metadata
        .get("model_tools_capable")
        .and_then(|v| v.as_bool().or_else(|| v.as_str().and_then(parse_bool_ish)))
    {
        return b;
    }
    let capable = model_supports_tools(metadata).await;
    metadata.insert(
        "model_tools_capable".to_string(),
        serde_json::Value::Bool(capable),
    );
    capable
}

/// Metadata key holding the JSON-serialized resolved available-files set.
const META_AVAILABLE_FILES: &str = "available_files";
/// Metadata key holding whether a non-empty available-files set resolved.
const META_FILES_MANIFEST_AVAILABLE: &str = "files_manifest_available";

/// Resolve the conversation's available files ONCE per LLM iteration and seed
/// them into the chat `metadata` (both a bool gate and the serialized set).
/// Called from the chat streaming loop BEFORE building the history-replay
/// transform context, so the value is shared by the file extension's
/// `before_llm_call` (which renders the manifest from it) AND
/// `process_content_for_llm` (which gates the recency-drop on it). Sharing one
/// resolution means the two can never disagree — a resolve failure seeds an
/// empty set (`manifest_available=false`), which degrades the drop to the safe
/// inline path instead of dropping content with no manifest to recover it.
pub async fn seed_available_files(
    metadata: &mut std::collections::HashMap<String, serde_json::Value>,
    conversation_id: Uuid,
    user_id: Uuid,
) {
    let files = resolve_available_files(conversation_id, user_id)
        .await
        .unwrap_or_default();
    metadata.insert(
        META_FILES_MANIFEST_AVAILABLE.to_string(),
        serde_json::Value::Bool(!files.is_empty()),
    );
    if let Ok(v) = serde_json::to_value(&files) {
        metadata.insert(META_AVAILABLE_FILES.to_string(), v);
    }
}

/// Read the seeded available-files set back out of `metadata` (empty when
/// absent/unparseable — `files_manifest_available` is then false too).
pub fn available_files_from_metadata(
    metadata: &std::collections::HashMap<String, serde_json::Value>,
) -> Vec<AvailableFile> {
    metadata
        .get(META_AVAILABLE_FILES)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// Whether a non-empty available-files set resolved this iteration (gates the
/// replay recency-drop — never drop when the manifest is unavailable).
pub fn files_manifest_available(
    metadata: &std::collections::HashMap<String, serde_json::Value>,
) -> bool {
    metadata
        .get(META_FILES_MANIFEST_AVAILABLE)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Best-effort *effective* context window (in tokens) for the chat model
/// described by `metadata`. Local models: `min(native context_length, runtime
/// ctx_size)` — the native trained length parsed at validation (persisted on
/// `capabilities`) capped by the KV-cache window the engine actually launched
/// with (llama.cpp `--ctx-size`). A model trained for 32K but launched with
/// `ctx_size=8192` only attends to 8192 tokens; a `ctx_size` above native runs
/// but degrades past the trained length, so native still caps the useful
/// window. Cloud models: the curated `model_registry` `context_length` (keyed
/// by provider_type + model NAME). `None` when unknown. Used by the token-aware
/// summarizer to compute a fraction-of-window trigger.
pub async fn model_context_window(
    metadata: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<u32> {
    if let Some(model_id) = metadata
        .get("model_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        let model = match Repos.llm_model.get_by_id(model_id).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    "model lookup failed during context-window resolution: {e}"
                );
                None
            }
        };
        if let Some(model) = model {
            let native = model.capabilities.context_length;
            // llama.cpp runtime KV-cache window. mistral.rs has no ctx override,
            // so only llamacpp contributes a cap here.
            let ctx_size = model
                .engine_settings
                .as_ref()
                .and_then(|e| e.llamacpp.as_ref())
                .and_then(|l| l.ctx_size)
                .filter(|&c| c > 0)
                .map(|c| c as u32);
            let effective = match (native, ctx_size) {
                (Some(n), Some(c)) => Some(n.min(c)),
                (Some(n), None) => Some(n),
                (None, Some(c)) => Some(c),
                (None, None) => None,
            };
            // Only return when we actually learned a window; otherwise fall
            // through to the curated catalog (e.g. a local row missing both).
            if let Some(w) = effective {
                return Some(w);
            }
        }
    }
    if let (Some(ptype), Some(mname)) = (
        metadata.get("provider_type").and_then(|v| v.as_str()),
        metadata.get("model_name").and_then(|v| v.as_str()),
    ) {
        if let Some(caps) = ai_providers::model_registry::lookup(ptype, mname) {
            return caps.context_length;
        }
    }
    None
}

/// Render the compact manifest injected as a system block when the model is
/// tool-capable. The model reads files on demand via the `files` MCP tools.
pub fn render_manifest(files: &[AvailableFile]) -> String {
    let mut s = String::from(
        "## Files available in this conversation\n\
         These files are NOT included in full below — read them on demand with the \
         file tools: `list_files()`, `read_file(id, offset?, limit?)`, \
         `grep_files(pattern, id?)`. Address files by `id` (preferred); a \
         `name` works only when it uniquely identifies one file. \
         Use ONLY the ids listed here (or from `list_files()`) — an id from \
         anywhere else, such as another tool's output or a URL, is not valid \
         here. \
         Files marked `no-text` can't be read as text.\n\n",
    );
    for f in files {
        let kind = match f.file_type {
            FileType::Text => "text",
            FileType::Document => "document",
            FileType::Image => "image",
            FileType::Binary => "binary",
        };
        let readable = if f.text {
            // Documents read by PAGE (lines for plain text), so a 1-page doc
            // still says "1 page" — the read unit, not the byte count, is what
            // the model needs to size its `offset`/`limit`. Non-document text
            // reads by line and is simply labelled "text".
            if f.file_type == FileType::Document {
                let n = f.pages.max(1);
                format!("{} page{}", n, if n == 1 { "" } else { "s" })
            } else {
                "text".to_string()
            }
        } else if f.file_type == FileType::Image {
            "image (use vision)".to_string()
        } else {
            "no-text".to_string()
        };
        let sources: Vec<&str> = f
            .source
            .iter()
            .map(|s| match s {
                FileSource::Project => "project",
                FileSource::Attachment => "attachment",
                FileSource::Generated => "generated",
            })
            .collect();
        let aka = if f.aka.is_empty() {
            String::new()
        } else {
            format!(" (aka {})", f.aka.join(", "))
        };
        s.push_str(&format!(
            "- id={} · {}{} · {} · {} · {}B · {}\n",
            f.id,
            f.name,
            aka,
            kind,
            readable,
            f.size,
            sources.join("+"),
        ));
    }
    s
}

/// File ids the MODEL authored in this conversation, oldest first.
///
/// "Model-authored" = `files.created_by IN ('mcp','llm')` (migration 34's
/// vocabulary: `mcp` = an MCP server / the built-in `files` tools, `llm` = a
/// code-sandbox `save_as_artifact`; `user` and `workflow` are deliberately
/// excluded). Conversation scope comes from PROVENANCE —
/// `file_versions.source_message_id` (migration 93; no FK by design, so it
/// survives message edits / branch pruning) joined back to the conversation's
/// branch messages. Spans ALL branches, matching the attachment query.
///
/// Single source of truth for this set, shared by `resolve_available_files`
/// (the `files` MCP manifest) and `deliverables.rs` (the user-facing derived
/// list) so the two can never drift apart. `ORDER BY f.created_at, f.id` makes
/// the order DETERMINISTIC — the content-dedup canonical pick depends on it
/// (see `dedup_by_checksum`), and it also stabilizes the deliverables list.
pub(crate) async fn model_authored_file_ids(
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    // Dedup the provenance ids in a CTE, then JOIN `files` and ORDER BY there —
    // `SELECT DISTINCT` cannot ORDER BY a column outside its select list, and
    // this mirrors the attachment query's shape directly above.
    sqlx::query_scalar!(
        r#"
        WITH ids AS (
            SELECT DISTINCT fv.file_id
            FROM file_versions fv
            JOIN branch_messages bm ON bm.message_id = fv.source_message_id
            JOIN branches br ON br.id = bm.branch_id
            WHERE br.conversation_id = $1
        )
        SELECT f.id AS "file_id!"
        FROM ids
        JOIN files f ON f.id = ids.file_id
        WHERE f.user_id = $2
          AND f.created_by IN ('mcp', 'llm')
        ORDER BY f.created_at, f.id
        "#,
        conversation_id,
        user_id
    )
    .fetch_all(Repos.pool())
    .await
    .map_err(AppError::database_error)
}

/// Resolve the conversation's effective, deduped file set for `user_id`.
///
/// Ownership is enforced in one batched query (`get_by_ids_and_user`), so
/// foreign or deleted files never appear. Order: project files first (each by
/// upload time), then attachments (each by upload time via the query's
/// `ORDER BY created_at, id`), then model-authored files (same ordering), then
/// content-dedup collapses duplicates.
pub async fn resolve_available_files(
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<AvailableFile>, AppError> {
    // 1. Project files (if the conversation is filed under a project).
    let project_id = Repos
        .project
        .project_id_for_conversation(conversation_id)
        .await?;
    let project_file_ids: Vec<Uuid> = match project_id {
        Some(pid) => Repos.project_files.list_file_ids(pid).await?,
        None => Vec::new(),
    };

    // 2. Attachment file_ids — across ALL branches of the conversation
    //    (cross-message: a file referenced from any message is available).
    //    The `file_id` lives in `message_contents.content` JSONB under either
    //    `file_id` or `source.file_id`. Malformed UUIDs are filtered before the
    //    cast (user-influenced JSON), mirroring the sandbox query. Dedup the ids
    //    in a CTE, then JOIN `files` and `ORDER BY f.created_at, f.id` so the
    //    attachment order is DETERMINISTIC + stable across calls (the `f.id`
    //    tiebreaker keeps things stable when several files share a created_at) —
    //    mirrors `code_sandbox/repository.rs::get_conversation_files`. Without an
    //    ORDER BY the hash-aggregated DISTINCT returns rows in arbitrary order,
    //    which would flip the content-dedup canonical pick between turns.
    let attachment_rows = sqlx::query!(
        r#"
        WITH refs AS (
            SELECT COALESCE(
                mc.content ->> 'file_id',
                mc.content -> 'source' ->> 'file_id'
            ) AS fid
            FROM conversations c
            JOIN branches b ON b.conversation_id = c.id
            JOIN branch_messages bm ON bm.branch_id = b.id
            JOIN message_contents mc ON mc.message_id = bm.message_id
            WHERE c.id = $1
              AND mc.content_type IN ('file_attachment', 'image')
        ),
        ids AS (
            SELECT DISTINCT fid::uuid AS file_id
            FROM refs
            WHERE fid ~ $2
        )
        SELECT f.id AS "file_id!"
        FROM ids
        JOIN files f ON f.id = ids.file_id
        ORDER BY f.created_at, f.id
        "#,
        conversation_id,
        UUID_RE,
    )
    .fetch_all(Repos.pool())
    .await
    .map_err(AppError::database_error)?;
    let attachment_file_ids: Vec<Uuid> = attachment_rows.into_iter().map(|r| r.file_id).collect();

    // 3. Model-authored file ids — files the model produced in THIS conversation
    //    (create_file / convert_document / an ingested tool-result artifact).
    //    They carry no `file_attachment` row, so they're resolved by provenance;
    //    without this the tools would hand the model an id and then reject it.
    let authored_file_ids = model_authored_file_ids(conversation_id, user_id).await?;

    // 4. Batch-load + ownership-check the union of all three id lists in a SINGLE
    //    round-trip, then build `AvailableFile`s by walking the ordered id lists
    //    (project first) so the canonical-source preference and the content-dedup
    //    canonical pick stay stable. Foreign/deleted ids never make it into the
    //    map, so they silently drop out — same filtering as the old per-id
    //    `get_by_id_and_user` loop, minus N sequential queries.
    let mut union_ids: Vec<Uuid> = Vec::with_capacity(
        project_file_ids.len() + attachment_file_ids.len() + authored_file_ids.len(),
    );
    union_ids.extend(project_file_ids.iter().copied());
    union_ids.extend(attachment_file_ids.iter().copied());
    union_ids.extend(authored_file_ids.iter().copied());
    let files = Repos.file.get_by_ids_and_user(&union_ids, user_id).await?;
    let by_uuid: std::collections::HashMap<Uuid, File> =
        files.into_iter().map(|f| (f.id, f)).collect();

    let mut by_id: Vec<AvailableFile> = Vec::new();
    let mut seen: std::collections::HashMap<Uuid, usize> = std::collections::HashMap::new();

    // Generated LAST: appending keeps every pre-existing set's canonical pick
    // (project → attachment) byte-identical to before this source existed.
    for (ids, source) in [
        (project_file_ids, FileSource::Project),
        (attachment_file_ids, FileSource::Attachment),
        (authored_file_ids, FileSource::Generated),
    ] {
        for fid in ids {
            if let Some(&idx) = seen.get(&fid) {
                by_id[idx].add_source(source);
                continue;
            }
            // Ownership + existence check happened in the batch query;
            // dangling/foreign refs simply aren't in the map.
            let Some(file) = by_uuid.get(&fid) else {
                continue;
            };
            seen.insert(fid, by_id.len());
            by_id.push(AvailableFile::from_file(file, source));
        }
    }

    // 4. Content-dedup by (checksum). NULL checksums stay distinct.
    Ok(dedup_by_checksum(by_id))
}

/// Collapse byte-identical files (same `checksum`) into one entry. The canonical
/// entry is the FIRST in resolution order — project files first, then
/// attachments, then model-authored, each ordered by upload time — a stable
/// choice now that every source query has an explicit `ORDER BY created_at, id`;
/// later same-content files fold their filename into `aka`, their id into
/// `aka_ids`, and union their sources.
/// NULL-checksum files are never merged (we can't prove identity).
///
/// The absorbed id MUST be kept (`aka_ids`): the model may already hold it from
/// a tool result (`"Created 'x' (id …)"`), and dropping it made a legitimately-
/// issued id resolve to nothing — the same "no longer available" dead end this
/// module's third source fixes.
fn dedup_by_checksum(files: Vec<AvailableFile>) -> Vec<AvailableFile> {
    use std::collections::HashMap;
    let mut canonical: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<AvailableFile> = Vec::new();

    for f in files {
        let Some(idx) = f.checksum.as_ref().and_then(|s| canonical.get(s).copied()) else {
            if let Some(sum) = f.checksum.clone() {
                canonical.insert(sum, out.len());
            }
            out.push(f);
            continue;
        };
        let c = &mut out[idx];
        if f.name != c.name && !c.aka.contains(&f.name) {
            c.aka.push(f.name);
        }
        if f.id != c.id && !c.aka_ids.contains(&f.id) {
            c.aka_ids.push(f.id);
        }
        for s in f.source {
            c.add_source(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn text_like_and_doc_like_classification() {
        assert!(is_text_like("text/markdown"));
        assert!(is_text_like("application/json"));
        assert!(!is_text_like("application/pdf"));
        assert!(is_doc_like("application/pdf"));
        assert!(is_doc_like(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        ));
        assert!(!is_doc_like("text/plain"));
    }

    fn mk(id: Uuid, name: &str, checksum: Option<&str>, src: FileSource) -> AvailableFile {
        AvailableFile {
            id,
            name: name.to_string(),
            aka: vec![],
            aka_ids: vec![],
            mime_type: Some("text/plain".into()),
            file_type: FileType::Text,
            text: true,
            size: 10,
            pages: 1,
            source: vec![src],
            uploaded_at: Utc::now(),
            checksum: checksum.map(|s| s.to_string()),
            blob_version_id: id,
        }
    }

    #[test]
    fn dedup_collapses_identical_checksum_with_alias() {
        let a = mk(Uuid::new_v4(), "report.md", Some("abc"), FileSource::Project);
        let b = mk(Uuid::new_v4(), "report-copy.md", Some("abc"), FileSource::Attachment);
        let out = dedup_by_checksum(vec![a.clone(), b]);
        assert_eq!(out.len(), 1, "identical checksum collapses to one");
        assert_eq!(out[0].id, a.id, "first-seen is canonical");
        assert!(out[0].aka.contains(&"report-copy.md".to_string()), "alias preserved");
        assert!(out[0].source.contains(&FileSource::Project));
        assert!(out[0].source.contains(&FileSource::Attachment), "sources unioned");
    }

    /// Content-dedup drops the absorbed file's ROW, but the model may already
    /// hold its id (a tool result handed it over before the duplicate appeared).
    /// That id must stay resolvable via `aka_ids` — otherwise a legitimately-
    /// issued id hits "not a file in this conversation". Mirrors the `aka`
    /// alias-name guarantee asserted above.
    #[test]
    fn dedup_preserves_absorbed_id_as_alias() {
        let a = mk(Uuid::new_v4(), "report.md", Some("abc"), FileSource::Project);
        let b = mk(Uuid::new_v4(), "report-copy.md", Some("abc"), FileSource::Generated);
        let b_id = b.id;
        let out = dedup_by_checksum(vec![a.clone(), b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, a.id, "first-seen stays canonical");
        assert!(
            out[0].aka_ids.contains(&b_id),
            "the absorbed file's id must remain resolvable as an alias; aka_ids={:?}",
            out[0].aka_ids
        );
    }

    /// A unique file absorbs nothing, so it carries no alias ids.
    #[test]
    fn dedup_leaves_aka_ids_empty_when_content_is_unique() {
        let a = mk(Uuid::new_v4(), "a.md", Some("h1"), FileSource::Project);
        let b = mk(Uuid::new_v4(), "b.md", Some("h2"), FileSource::Attachment);
        let out = dedup_by_checksum(vec![a, b]);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|f| f.aka_ids.is_empty()));
    }

    /// `aka_ids` is internal plumbing: surfacing it would show the model two ids
    /// for one file — the exact confusion this module exists to prevent. It must
    /// never appear in the `list_files` payload (which is this struct's serde).
    #[test]
    fn aka_ids_and_checksum_are_never_serialized_to_the_model() {
        let a = mk(Uuid::new_v4(), "report.md", Some("abc"), FileSource::Project);
        let b = mk(Uuid::new_v4(), "copy.md", Some("abc"), FileSource::Generated);
        let out = dedup_by_checksum(vec![a, b]);
        let json = serde_json::to_value(&out[0]).unwrap();
        assert!(json.get("aka_ids").is_none(), "aka_ids must not reach the model: {json}");
        assert!(json.get("checksum").is_none());
        assert!(json.get("blob_version_id").is_none());
        // The canonical id + alias NAME are still surfaced, as before.
        assert!(json.get("id").is_some());
        assert_eq!(json["aka"][0], "copy.md");
    }

    /// The `Generated` source renders in the injected manifest (a new arm in
    /// `render_manifest`'s exhaustive match).
    #[test]
    fn manifest_renders_generated_source() {
        let f = mk(Uuid::new_v4(), "authored.md", Some("h1"), FileSource::Generated);
        let s = render_manifest(&[f]);
        assert!(s.contains("generated"), "manifest must label the source; got:\n{s}");
        assert!(s.contains("authored.md"));
    }

    #[test]
    fn dedup_keeps_null_checksum_distinct() {
        let a = mk(Uuid::new_v4(), "a.txt", None, FileSource::Attachment);
        let b = mk(Uuid::new_v4(), "b.txt", None, FileSource::Attachment);
        assert_eq!(dedup_by_checksum(vec![a, b]).len(), 2);
    }

    #[test]
    fn dedup_keeps_different_checksum_distinct() {
        let a = mk(Uuid::new_v4(), "x", Some("h1"), FileSource::Attachment);
        let b = mk(Uuid::new_v4(), "x", Some("h2"), FileSource::Attachment);
        assert_eq!(dedup_by_checksum(vec![a, b]).len(), 2, "same name diff content stay distinct");
    }

    /// `model_context_window` resolves the chat model's window from the curated
    /// catalog when metadata carries `provider_type` + `model_name` (and no
    /// `model_id`, so no DB lookup). This window is the INPUT to summarization's
    /// fraction-of-window trigger override (`0.75 × window`); assert both the
    /// resolved window and the derived override.
    #[tokio::test]
    async fn model_context_window_resolves_from_catalog_and_drives_075_override() {
        use std::collections::HashMap;

        let mut metadata: HashMap<String, serde_json::Value> = HashMap::new();
        metadata.insert("provider_type".into(), serde_json::json!("openai"));
        metadata.insert("model_name".into(), serde_json::json!("gpt-4o"));

        let window = model_context_window(&metadata).await;
        assert_eq!(
            window,
            Some(128000),
            "gpt-4o window must resolve from the curated catalog"
        );

        // Mirror summarization::chat_extension::summarization.rs's override math.
        let override_trigger = window.map(|w| (w as f64 * 0.75) as usize);
        assert_eq!(
            override_trigger,
            Some(96000),
            "fraction-of-window override must be 0.75 × the resolved window"
        );
    }

    /// With neither a resolvable `model_id` nor a catalog-known
    /// provider_type/model_name, the window is unknown → `None`, so no
    /// fraction-of-window override is applied (the admin flat threshold stands).
    #[tokio::test]
    async fn model_context_window_unknown_model_is_none() {
        use std::collections::HashMap;
        let mut metadata: HashMap<String, serde_json::Value> = HashMap::new();
        metadata.insert("provider_type".into(), serde_json::json!("openai"));
        metadata.insert("model_name".into(), serde_json::json!("no-such-model-xyz"));
        assert_eq!(model_context_window(&metadata).await, None);
    }
}
