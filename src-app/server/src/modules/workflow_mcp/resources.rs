//! `resources/list` + `resources/read` for the built-in workflow MCP
//! server (plan §4.7).
//!
//! Three kinds of resource, all under the `ziee://workflow-runs/...`
//! scheme, scoped to the requesting user's recent runs:
//!
//! - **Outputs**: `ziee://workflow-runs/<run>/outputs/<name>` — an
//!   `outputs[]` entry whose `expose` resolves to `artifact` (explicit
//!   or auto-promoted because it's over the 4 KiB inline cap).
//! - **Artifacts**: `ziee://workflow-runs/<run>/artifacts/<step>/<file>`
//!   — every file a step collected into `artifacts/<step>/`.
//! - **Logs**: `ziee://workflow-runs/<run>/logs/<step>/<kind>` — per-step
//!   diagnostic captures (`prompt|raw_output|stderr|items|trace`), gated
//!   by the workflow's `expose_logs:` + the per-step override.
//!
//! `resources/read` parses all three forms, re-validates ownership
//! (`workflow_runs.user_id == JWT sub`), reads from the on-disk staged
//! run dir (reusing the same path-reconstruction the REST stream
//! handlers use), and returns MCP `Content` (text for text mimes,
//! base64 for binary). 404 when cleaned up or `expose_logs: never`
//! excludes a log.

#![allow(dead_code)]

use base64::Engine as _;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::workflow::models::WorkflowRun;
use crate::modules::workflow::repository;
use crate::modules::workflow::runner::workflow_workspace_root;
use crate::modules::workflow::types::{ArtifactMeta, OutputMeta};
use crate::modules::workflow::validate::{ExposeLogs, ExposeMode, WorkflowDef, parse_workflow_yaml};

use super::tools::INLINE_FULL_CAP_BYTES;

/// How many recent runs to surface in `resources/list`.
const RECENT_RUN_LIMIT: i64 = 50;
/// Per-resource read cap (plan §4.7: per-resource 1 MiB cap on logs).
const READ_CAP_BYTES: u64 = 1024 * 1024;

const LOG_KINDS: &[&str] = &["prompt", "raw_output", "stderr", "items", "trace"];

// ── path-safety (SEC-1) ────────────────────────────────────────────────

/// Reject any URI path component that could traverse out of the run dir:
/// `..`, empty, absolute markers, or embedded path separators (`/`, `\`).
/// Mirrors `skill_mcp::sanitize_relative_path`'s airtight component check
/// (per the security audit) but applied per-segment, since these segments
/// come straight off an attacker-controllable `ziee://` URI. The URI is
/// already URL-decoded by the transport, so a `%2f`-encoded separator
/// arrives here as a literal `/` and is caught by the separator check.
fn sanitize_uri_component(label: &str, seg: &str) -> Result<String, AppError> {
    if seg.is_empty()
        || seg == ".."
        || seg == "."
        || seg.contains('/')
        || seg.contains('\\')
        || seg.contains('\0')
    {
        return Err(AppError::bad_request(
            "WORKFLOW_URI_INVALID",
            format!("{label} component '{seg}' is not a safe path segment"),
        ));
    }
    Ok(seg.to_string())
}

/// Defense-in-depth: after building a path under the run dir, canonicalize
/// it (and the run dir) and confirm the resolved file stays under the run
/// workspace dir — catches symlink escapes + any residual traversal. The
/// run dir is `<workspace>/<conv-or-run>/workflow/<run>/`.
fn confirm_under_run_dir(run: &WorkflowRun, path: &std::path::Path) -> Result<(), AppError> {
    let conv_dir_id = run.conversation_id.unwrap_or(run.id);
    let run_dir = workflow_workspace_root()
        .join(conv_dir_id.to_string())
        .join("workflow")
        .join(run.id.to_string());
    let canon_root = std::fs::canonicalize(&run_dir)
        .map_err(|e| AppError::not_found(&format!("workflow run dir missing: {e}")))?;
    let canon_path = std::fs::canonicalize(path)
        .map_err(|_| AppError::not_found("resource file not found in run dir"))?;
    if !canon_path.starts_with(&canon_root) {
        return Err(AppError::forbidden(
            "WORKFLOW_PATH_ESCAPE",
            "resolved path escapes the workflow run dir",
        ));
    }
    Ok(())
}

// ── resources/list ────────────────────────────────────────────────────

pub async fn resources_list(pool: &sqlx::PgPool, user_id: Uuid) -> Result<Value, AppError> {
    let runs = repository::list_runs_for_user(pool, user_id, RECENT_RUN_LIMIT).await?;
    let mut resources: Vec<Value> = Vec::new();

    for run in runs {
        // The workflow def gives us outputs[] expose modes + expose_logs.
        let def = workflow_def_for_run(pool, &run).await.ok();

        // 1. Outputs that resolve to artifact (explicit or auto-promoted).
        if let Some(obj) = run.final_output_json.as_ref().and_then(|v| v.as_object()) {
            for (name, meta) in obj {
                let size_bytes = meta
                    .get("size_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let expose = def
                    .as_ref()
                    .and_then(|d| d.outputs.iter().find(|o| &o.name == name))
                    .map(|o| o.expose)
                    .unwrap_or(ExposeMode::Full);
                let is_resource = matches!(expose, ExposeMode::Artifact)
                    || (matches!(expose, ExposeMode::Full)
                        && size_bytes as usize > INLINE_FULL_CAP_BYTES);
                if !is_resource {
                    continue;
                }
                let mime = def
                    .as_ref()
                    .and_then(|d| d.outputs.iter().find(|o| &o.name == name))
                    .and_then(|o| o.mime_type.clone())
                    .unwrap_or_else(|| "text/plain".to_string());
                resources.push(json!({
                    "uri": super::tools::output_uri(run.id, name),
                    "name": name,
                    "description": format!("Workflow output '{name}' ({size_bytes} bytes)"),
                    "mimeType": mime,
                }));
            }
        }

        // 2. Artifacts — every collected file per step.
        if let Some(obj) = run.step_artifacts_json.as_object() {
            for (step_id, arts) in obj {
                let metas: Vec<ArtifactMeta> =
                    serde_json::from_value(arts.clone()).unwrap_or_default();
                for m in metas {
                    resources.push(json!({
                        "uri": artifact_uri(run.id, step_id, &m.filename),
                        "name": m.filename,
                        "description": m.description.clone().unwrap_or_else(|| {
                            format!("Artifact from step '{step_id}' ({} bytes)", m.size_bytes)
                        }),
                        "mimeType": m.mime_type,
                    }));
                }
            }
        }

        // 3. Logs — per-step, gated by expose_logs. L11: list only the
        //    kinds actually captured for the step (the per-step object's
        //    keys), not all of LOG_KINDS — listing a kind that was never
        //    captured would 404 on resources/read.
        if let Some(obj) = run.step_logs_json.as_object() {
            for (step_id, step_logs) in obj {
                if !logs_surfaceable(def.as_ref(), step_id) {
                    continue;
                }
                let captured = step_logs.as_object();
                for kind in LOG_KINDS {
                    // Only advertise a kind that was actually captured. If the
                    // per-step value isn't an object we can't tell, so skip.
                    let present = captured.map(|c| c.contains_key(*kind)).unwrap_or(false);
                    if !present {
                        continue;
                    }
                    resources.push(json!({
                        "uri": log_uri(run.id, step_id, kind),
                        "name": format!("{step_id} · {kind}"),
                        "description": format!("Diagnostic log ({kind}) for step '{step_id}'"),
                        "mimeType": if *kind == "trace" { "application/json" } else { "text/plain" },
                    }));
                }
            }
        }
    }

    Ok(json!({ "resources": resources }))
}

// ── resources/read ────────────────────────────────────────────────────

pub async fn resources_read(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    uri: &str,
) -> Result<Value, AppError> {
    let parsed = parse_uri(uri)?;
    let run = repository::find_run(pool, parsed.run_id)
        .await?
        .ok_or_else(|| AppError::not_found("workflow run not found or cleaned up"))?;
    if run.user_id != user_id {
        return Err(AppError::forbidden(
            "WORKFLOW_RUN_FORBIDDEN",
            "workflow run is owned by another user",
        ));
    }

    let (bytes, mime) = match parsed.kind {
        ResourceKind::Output(name) => {
            // C2: an output's `from` may reference a step whose id differs
            // from the output name, so we need the def to resolve the
            // backing step file (mirrors tools::read_full_output_value).
            let def = workflow_def_for_run(pool, &run).await.ok();
            read_output(&run, def.as_ref(), &name)?
        }
        ResourceKind::Artifact { step_id, filename } => {
            read_artifact(&run, &step_id, &filename)?
        }
        ResourceKind::Log { step_id, kind } => {
            // Gate by expose_logs.
            let def = workflow_def_for_run(pool, &run).await.ok();
            if !logs_surfaceable(def.as_ref(), &step_id) {
                return Err(AppError::not_found(
                    "log resource excluded by expose_logs: never",
                ));
            }
            read_log(&run, &step_id, &kind)?
        }
    };

    if bytes.len() as u64 > READ_CAP_BYTES {
        return Err(AppError::bad_request(
            "WORKFLOW_RESOURCE_TOO_LARGE",
            format!("resource exceeds the {READ_CAP_BYTES}-byte read cap"),
        ));
    }

    // Text mimes return as text; everything else base64.
    let content = if is_text_mime(&mime) {
        match String::from_utf8(bytes) {
            Ok(text) => json!({ "uri": uri, "mimeType": mime, "text": text }),
            Err(e) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(e.into_bytes());
                json!({ "uri": uri, "mimeType": mime, "blob": b64 })
            }
        }
    } else {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        json!({ "uri": uri, "mimeType": mime, "blob": b64 })
    };

    Ok(json!({ "contents": [content] }))
}

// ── on-disk readers ───────────────────────────────────────────────────

fn read_output(
    run: &WorkflowRun,
    def: Option<&WorkflowDef>,
    name: &str,
) -> Result<(Vec<u8>, String), AppError> {
    // SEC-1: parse_uri already sanitized the name; re-check for any internal
    // caller (defense in depth — parity with read_log / read_artifact).
    sanitize_uri_component("output name", name)?;
    // C2: `step_outputs_json` is keyed by STEP ID, but the URI carries the
    // OUTPUT name. Resolve the source step id from the output's `from`
    // template (e.g. output `memo` ← `{{ synthesize.output }}` → step
    // `synthesize`); fall back to the name for the name==step-id case.
    let out_def = def.and_then(|d| d.outputs.iter().find(|o| o.name == name));
    let step_key = out_def
        .and_then(|o| super::tools::step_id_from_template(&o.from))
        .unwrap_or_else(|| name.to_string());
    let meta_json = run
        .step_outputs_json
        .get(&step_key)
        .or_else(|| run.step_outputs_json.get(name))
        .ok_or_else(|| AppError::not_found("output not found in this run"))?;
    let meta: OutputMeta = serde_json::from_value(meta_json.clone())
        .map_err(|e| AppError::internal_error(format!("decode output meta: {e}")))?;
    // H-1 defense-in-depth: confine the resolved path under the run dir
    // before reading (parity with read_artifact). The path is server-written
    // and the run dir is RO-mounted to the sandbox, so this is belt-and-
    // suspenders against a symlink escape, not a known-reachable hole.
    confirm_under_run_dir(run, &meta.path)?;
    let bytes = std::fs::read(&meta.path)
        .map_err(|e| AppError::not_found(&format!("output file missing: {e}")))?;
    // Prefer the output's declared mime_type (so list-mime and read-mime
    // agree), falling back to the file's parsed_as.
    let mime = out_def
        .and_then(|o| o.mime_type.clone())
        .unwrap_or_else(|| match meta.parsed_as {
            crate::modules::workflow::types::ParsedAs::Json => "application/json".to_string(),
            crate::modules::workflow::types::ParsedAs::Text => "text/plain".to_string(),
        });
    Ok((bytes, mime))
}

fn read_artifact(
    run: &WorkflowRun,
    step_id: &str,
    filename: &str,
) -> Result<(Vec<u8>, String), AppError> {
    // SEC-1: step_id + filename are already sanitized in `parse_uri`; this
    // re-check is defense in depth for any internal caller.
    sanitize_uri_component("artifact step id", step_id)?;
    sanitize_uri_component("artifact filename", filename)?;
    let step_arts = run
        .step_artifacts_json
        .get(step_id)
        .ok_or_else(|| AppError::not_found("step artifacts not found"))?;
    let arts: Vec<ArtifactMeta> = serde_json::from_value(step_arts.clone())
        .map_err(|e| AppError::internal_error(format!("decode artifact list: {e}")))?;
    let meta = arts
        .into_iter()
        .find(|m| m.filename == filename)
        .ok_or_else(|| AppError::not_found("artifact filename not found"))?;
    // Confirm the recorded host_path stays under the run dir before reading.
    confirm_under_run_dir(run, &meta.host_path)?;
    let bytes = std::fs::read(&meta.host_path)
        .map_err(|e| AppError::not_found(&format!("artifact file missing: {e}")))?;
    Ok((bytes, meta.mime_type))
}

fn read_log(
    run: &WorkflowRun,
    step_id: &str,
    kind: &str,
) -> Result<(Vec<u8>, String), AppError> {
    if !LOG_KINDS.contains(&kind) {
        return Err(AppError::bad_request(
            "WORKFLOW_LOG_BAD_KIND",
            format!("log kind '{kind}' not recognized"),
        ));
    }
    // SEC-1: re-sanitize the step id (defense in depth; parse_uri already
    // rejected separators / `..`, but read_log is also reachable internally).
    sanitize_uri_component("log step id", step_id)?;
    // Reconstruct the on-disk path the same way log_stream.rs does:
    // <workspace>/<conv-or-run>/workflow/<run>/logs/<step>/<kind>[.json]
    let conv_dir_id = run.conversation_id.unwrap_or(run.id);
    let base = workflow_workspace_root()
        .join(conv_dir_id.to_string())
        .join("workflow")
        .join(run.id.to_string())
        .join("logs")
        .join(step_id);
    let path = if kind == "trace" {
        base.join("trace.json")
    } else if kind == "items" {
        // Per-item llm_map traces live in an `items/` subdir; surface the
        // dir listing as a JSON index (individual items are addressable
        // via REST). Phase 1 returns the index.
        let items_dir = base.join("items");
        // H-1 defense-in-depth: confine before listing.
        confirm_under_run_dir(run, &items_dir)?;
        let mut names: Vec<String> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&items_dir) {
            for e in rd.flatten() {
                names.push(e.file_name().to_string_lossy().to_string());
            }
        }
        names.sort();
        let idx = json!({ "items": names });
        return Ok((
            serde_json::to_vec(&idx).unwrap_or_default(),
            "application/json".to_string(),
        ));
    } else {
        base.join(kind)
    };
    // H-1 defense-in-depth: confine the resolved log path under the run dir.
    confirm_under_run_dir(run, &path)?;
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => {
            // A7: the staging dir was reclaimed — fall back to the durable body
            // in step_logs_json. The `expose_logs`/`logs_surfaceable` gate is
            // applied by the caller (resources_read) before reaching here.
            match run
                .step_logs_json
                .get(step_id)
                .and_then(|m| m.get(kind))
                .and_then(|e| e.get("body"))
                .and_then(|b| b.as_str())
            {
                Some(s) => s.as_bytes().to_vec(),
                None => return Err(AppError::not_found("log no longer available")),
            }
        }
    };
    let mime = if kind == "trace" {
        "application/json".to_string()
    } else {
        "text/plain".to_string()
    };
    Ok((bytes, mime))
}

// ── URI parsing + building ────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Output(String),
    Artifact { step_id: String, filename: String },
    Log { step_id: String, kind: String },
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedResourceUri {
    pub run_id: Uuid,
    pub kind: ResourceKind,
}

pub fn artifact_uri(run_id: Uuid, step_id: &str, filename: &str) -> String {
    format!("ziee://workflow-runs/{run_id}/artifacts/{step_id}/{filename}")
}

pub fn log_uri(run_id: Uuid, step_id: &str, kind: &str) -> String {
    format!("ziee://workflow-runs/{run_id}/logs/{step_id}/{kind}")
}

/// Parse any of the three `ziee://workflow-runs/<run>/...` URI forms.
pub fn parse_uri(uri: &str) -> Result<ParsedResourceUri, AppError> {
    const PREFIX: &str = "ziee://workflow-runs/";
    let rest = uri.strip_prefix(PREFIX).ok_or_else(|| {
        AppError::bad_request(
            "WORKFLOW_URI_INVALID",
            format!("URI must start with '{PREFIX}'"),
        )
    })?;
    let mut parts = rest.split('/');
    let run_str = parts.next().ok_or_else(|| {
        AppError::bad_request("WORKFLOW_URI_INVALID", "missing run id")
    })?;
    let run_id = Uuid::parse_str(run_str).map_err(|_| {
        AppError::bad_request("WORKFLOW_URI_INVALID", "run id is not a uuid")
    })?;
    let category = parts.next().ok_or_else(|| {
        AppError::bad_request("WORKFLOW_URI_INVALID", "missing resource category")
    })?;

    let kind = match category {
        "outputs" => {
            let name = parts.next().ok_or_else(|| {
                AppError::bad_request("WORKFLOW_URI_INVALID", "missing output name")
            })?;
            if parts.next().is_some() {
                return Err(AppError::bad_request(
                    "WORKFLOW_URI_INVALID",
                    "trailing segments after output name",
                ));
            }
            // SEC-1: output name is later used to key step_outputs_json (no
            // path join) but sanitize anyway so a `..`/separator can't slip
            // into any downstream path use.
            ResourceKind::Output(sanitize_uri_component("output name", name)?)
        }
        "artifacts" => {
            let step_id = parts.next().ok_or_else(|| {
                AppError::bad_request("WORKFLOW_URI_INVALID", "missing artifact step id")
            })?;
            // Artifacts are flat per step — exactly one filename segment.
            let filename = parts.next().ok_or_else(|| {
                AppError::bad_request("WORKFLOW_URI_INVALID", "missing artifact filename")
            })?;
            if parts.next().is_some() {
                return Err(AppError::bad_request(
                    "WORKFLOW_URI_INVALID",
                    "trailing segments after artifact filename",
                ));
            }
            // SEC-1: sanitize BOTH the step id AND the filename (the old code
            // checked only the filename, leaving step_id traversable).
            ResourceKind::Artifact {
                step_id: sanitize_uri_component("artifact step id", step_id)?,
                filename: sanitize_uri_component("artifact filename", filename)?,
            }
        }
        "logs" => {
            let step_id = parts.next().ok_or_else(|| {
                AppError::bad_request("WORKFLOW_URI_INVALID", "missing log step id")
            })?;
            // kind is optional in list-form (`.../logs/<step>`); read-form
            // needs it. Default to "trace".
            let kind = parts.next().unwrap_or("trace");
            if parts.next().is_some() {
                return Err(AppError::bad_request(
                    "WORKFLOW_URI_INVALID",
                    "trailing segments after log kind",
                ));
            }
            // SEC-1: sanitize the step id (the old code joined it raw into the
            // log path). `kind` is whitelist-validated in `read_log`, but
            // sanitize it too for defense in depth.
            ResourceKind::Log {
                step_id: sanitize_uri_component("log step id", step_id)?,
                kind: sanitize_uri_component("log kind", kind)?,
            }
        }
        other => {
            return Err(AppError::bad_request(
                "WORKFLOW_URI_INVALID",
                format!("unknown resource category '{other}'"),
            ));
        }
    };

    Ok(ParsedResourceUri { run_id, kind })
}

// ── helpers ───────────────────────────────────────────────────────────

async fn workflow_def_for_run(
    pool: &sqlx::PgPool,
    run: &WorkflowRun,
) -> Result<WorkflowDef, AppError> {
    let wf = repository::find_by_id(pool, run.workflow_id)
        .await?
        .ok_or_else(|| AppError::not_found("workflow"))?;
    let path = std::path::PathBuf::from(&wf.extracted_path).join(&wf.entry_point);
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| AppError::internal_error(format!("read workflow.yaml: {e}")))?;
    parse_workflow_yaml(&content)
}

/// Whether logs for `step_id` are surfaceable given the workflow's
/// `expose_logs` (per-step override wins). M-6: `None` def → fail CLOSED.
/// `expose_logs` is a confidentiality control (a step can mark its prompts /
/// raw output `never`); if the def can't be loaded we can't prove the step
/// isn't `never`, so we must not surface its logs. Ownership is still
/// enforced separately, so this only ever hides a same-user's own logs when
/// the workflow.yaml is unreadable — the safe trade-off for a privacy gate.
fn logs_surfaceable(def: Option<&WorkflowDef>, step_id: &str) -> bool {
    let Some(def) = def else { return false };
    let effective = def
        .steps
        .iter()
        .find(|s| s.id == step_id)
        .and_then(|s| s.expose_logs)
        .unwrap_or(def.expose_logs);
    !matches!(effective, ExposeLogs::Never)
}

fn is_text_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || mime == "application/json"
        || mime == "application/x-yaml"
        || mime == "application/yaml"
        || mime.ends_with("+json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_roundtrip_output() {
        let run = Uuid::new_v4();
        let uri = super::super::tools::output_uri(run, "summary");
        let parsed = parse_uri(&uri).unwrap();
        assert_eq!(parsed.run_id, run);
        assert_eq!(parsed.kind, ResourceKind::Output("summary".into()));
    }

    #[test]
    fn uri_roundtrip_artifact() {
        let run = Uuid::new_v4();
        let uri = artifact_uri(run, "render_report", "report.md");
        let parsed = parse_uri(&uri).unwrap();
        assert_eq!(parsed.run_id, run);
        assert_eq!(
            parsed.kind,
            ResourceKind::Artifact {
                step_id: "render_report".into(),
                filename: "report.md".into()
            }
        );
    }

    #[test]
    fn uri_roundtrip_log() {
        let run = Uuid::new_v4();
        let uri = log_uri(run, "extract", "raw_output");
        let parsed = parse_uri(&uri).unwrap();
        assert_eq!(parsed.run_id, run);
        assert_eq!(
            parsed.kind,
            ResourceKind::Log {
                step_id: "extract".into(),
                kind: "raw_output".into()
            }
        );
    }

    #[test]
    fn uri_rejects_bad_prefix_and_uuid() {
        assert!(parse_uri("http://workflow-runs/x/outputs/y").is_err());
        assert!(parse_uri("ziee://workflow-runs/not-a-uuid/outputs/y").is_err());
    }

    #[test]
    fn uri_rejects_unknown_category() {
        let run = Uuid::new_v4();
        let bad = format!("ziee://workflow-runs/{run}/bogus/x");
        assert!(parse_uri(&bad).is_err());
    }

    #[test]
    fn log_uri_without_kind_defaults_to_trace() {
        let run = Uuid::new_v4();
        let uri = format!("ziee://workflow-runs/{run}/logs/step1");
        let parsed = parse_uri(&uri).unwrap();
        assert_eq!(
            parsed.kind,
            ResourceKind::Log {
                step_id: "step1".into(),
                kind: "trace".into()
            }
        );
    }

    #[test]
    fn text_mime_classification() {
        assert!(is_text_mime("text/markdown"));
        assert!(is_text_mime("application/json"));
        assert!(!is_text_mime("image/png"));
        assert!(!is_text_mime("application/octet-stream"));
    }

    // ── SEC-1: path traversal in step_id / filename / output name ─────

    #[test]
    fn sanitize_rejects_traversal_segments() {
        assert!(sanitize_uri_component("x", "..").is_err());
        assert!(sanitize_uri_component("x", ".").is_err());
        assert!(sanitize_uri_component("x", "a/b").is_err());
        assert!(sanitize_uri_component("x", "a\\b").is_err());
        assert!(sanitize_uri_component("x", "").is_err());
        assert!(sanitize_uri_component("x", "a\0b").is_err());
        // A normal segment passes.
        assert_eq!(sanitize_uri_component("x", "step1").unwrap(), "step1");
        assert_eq!(sanitize_uri_component("x", "report.md").unwrap(), "report.md");
    }

    #[test]
    fn parse_uri_rejects_dotdot_in_log_step_id() {
        // SEC-1: `..` in the step_id of a log URI must be rejected. The
        // transport URL-decodes, so `..%2f..%2fetc` arrives as literal `/`
        // segments — each is caught either as the `..` segment or as an
        // embedded separator in the surrounding segment.
        let run = Uuid::new_v4();
        // Literal `..` segment in the step id position.
        let bad = format!("ziee://workflow-runs/{run}/logs/../stderr");
        assert!(parse_uri(&bad).is_err(), "must reject `..` step id");
    }

    #[test]
    fn parse_uri_rejects_dotdot_in_artifact_step_id() {
        let run = Uuid::new_v4();
        let bad = format!("ziee://workflow-runs/{run}/artifacts/../report.md");
        assert!(parse_uri(&bad).is_err(), "must reject `..` artifact step id");
    }

    #[test]
    fn parse_uri_rejects_dotdot_in_artifact_filename() {
        let run = Uuid::new_v4();
        // step ok, filename traversal — `..%2f..` decoded → `../..` which the
        // single-segment rule + `..` check reject (the extra `/` makes it a
        // trailing-segment error or the `..` check fires first).
        let bad = format!("ziee://workflow-runs/{run}/artifacts/step1/..");
        assert!(parse_uri(&bad).is_err(), "must reject `..` filename");
        let bad2 = format!("ziee://workflow-runs/{run}/artifacts/step1/a/../b");
        assert!(parse_uri(&bad2).is_err(), "must reject multi-segment filename");
    }

    #[test]
    fn parse_uri_rejects_dotdot_in_output_name() {
        let run = Uuid::new_v4();
        let bad = format!("ziee://workflow-runs/{run}/outputs/..");
        assert!(parse_uri(&bad).is_err(), "must reject `..` output name");
    }
}
