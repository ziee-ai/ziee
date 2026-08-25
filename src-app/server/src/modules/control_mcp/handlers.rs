//! JSON-RPC handler for the built-in control MCP server.
//!
//! Gated on `control::use` (`RequirePermissions`). Three tools:
//! `list_capabilities` / `describe_capability` / `invoke_capability`. The first
//! two are metadata reads over the in-process [`catalog`]; the third dispatches
//! to the REAL REST route over loopback, forwarding the caller's JWT so the
//! target route's own `RequirePermissions` re-authorizes from the DB — no authz
//! is reimplemented here.
//!
//! Precision comes from the catalog (operation_id → method/path/schema);
//! security from three layers: the deployment [`policy`] denylist, the per-user
//! permission filter applied to ALL THREE tools (the model never sees an op it
//! can't run), and the forwarded-JWT loopback call (the real gate).

use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use axum::{
    Json, debug_handler,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::common::AppError;
use crate::modules::code_sandbox::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::modules::permissions::RequirePermissions;
use crate::modules::permissions::checker::check_permission_union;
use crate::modules::user::models::{Group, User};

use super::catalog::{self, ControlCatalog, Operation};
use super::permissions::ControlUse;
use super::policy;
use super::schema_inline::{self, InlinedSchema, resolve_schema_ref};
use super::tools;

/// Cap on the response body we relay back to the model (mirrors the chat-path
/// tool-result caps). Larger responses are truncated with a marker.
const MAX_RESULT_BYTES: usize = 1024 * 1024;
/// Cap on how many operations `list_capabilities` returns in one call.
const MAX_LIST_RESULTS: usize = 200;

/// The loopback base URL (`http://<host>:<port>`) the invoke path dispatches to.
/// Set once at module init from the server config. Never model-supplied, so the
/// invoke target host is fixed — the model only controls the path/params of a
/// route that already exists in OUR catalog.
static CONTROL_BASE_URL: OnceLock<String> = OnceLock::new();

pub fn set_base_url(base: String) {
    let _ = CONTROL_BASE_URL.set(base);
}

/// One shared client for the loopback dispatch (per guidelines §2). Loopback
/// only, but we still bound it and refuse redirects (a REST route should not
/// 3xx us off-host).
///
/// Built lazily on the FIRST request, so a build failure maps to an AppError at
/// the dispatch site rather than panicking the worker. reqwest's build is
/// near-infallible (only TLS-backend init can fail) and deterministic, so
/// caching the `None` is fine.
static HTTP_CLIENT: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| {
            tracing::error!(error = %e, "control_mcp: failed to build loopback HTTP client");
        })
        .ok()
});

#[debug_handler]
pub async fn jsonrpc_handler(
    auth: RequirePermissions<(ControlUse,)>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let req: JsonRpcRequest = match serde_json::from_slice::<Value>(&body)
        .map_err(|e| JsonRpcError::parse_error(e.to_string()))
        .and_then(|raw| {
            serde_json::from_value(raw).map_err(|e| JsonRpcError::invalid_request(e.to_string()))
        }) {
        Ok(r) => r,
        Err(err) => return error_response(None, StatusCode::BAD_REQUEST, err),
    };

    // Notifications (no id) get an ACK, no body.
    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "control", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
        "tools/list" => ok_response(id, tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        "tools/call" => {
            let call: ToolCallParams = match serde_json::from_value(req.params.clone()) {
                Ok(c) => c,
                Err(e) => {
                    return error_response(
                        id,
                        StatusCode::OK,
                        JsonRpcError::invalid_params(format!("tools/call params: {e}")),
                    );
                }
            };
            let Some(catalog) = catalog::catalog() else {
                return error_response(
                    id,
                    StatusCode::OK,
                    JsonRpcError::internal(
                        "control catalog unavailable (server did not initialize it)".to_string(),
                    ),
                );
            };
            let result = match call.name.as_str() {
                tools::LIST_CAPABILITIES => {
                    list_capabilities(&auth.user, &auth.groups, catalog, &call.arguments)
                }
                tools::DESCRIBE_CAPABILITY => {
                    describe_capability(&auth.user, &auth.groups, catalog, &call.arguments)
                }
                tools::INVOKE_CAPABILITY => {
                    invoke_capability(&auth.user, &auth.groups, catalog, &headers, &call.arguments)
                        .await
                }
                other => Err(AppError::bad_request(
                    "UNKNOWN_TOOL",
                    format!("control tool: {other}"),
                )),
            };
            match result {
                Ok(value) => ok_response(id, value),
                Err(e) => error_response(id, StatusCode::OK, JsonRpcError::from_app_error(&e)),
            }
        }
        _ => error_response(id, StatusCode::OK, JsonRpcError::method_not_found(&req.method)),
    }
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

/// Decide whether a control tool call must go through the explicit approval
/// prompt. Called from `mcp/chat_extension/mcp.rs`'s classification loop — the
/// control server is deliberately NOT on the blanket approval-bypass list, so
/// this per-tool rule governs it:
/// - `list_capabilities` / `describe_capability` → read-only metadata, auto-run.
/// - `invoke_capability` of a GET operation → read-only, auto-run.
/// - `invoke_capability` of a mutating operation → ALWAYS approve (even under
///   `ApprovalMode::AutoApprove` — that's the security posture).
/// - anything unrecognized (unknown tool / unknown op / catalog unavailable) →
///   approve (fail-safe).
pub fn control_call_needs_approval(tool_name: &str, input: &Value) -> bool {
    needs_approval_decision(tool_name, input, catalog::catalog())
}

/// Pure core of [`control_call_needs_approval`] (catalog injected) so the
/// security-critical decision is unit-testable without the global `OnceLock`.
fn needs_approval_decision(
    tool_name: &str,
    input: &Value,
    catalog: Option<&ControlCatalog>,
) -> bool {
    match tool_name {
        tools::LIST_CAPABILITIES | tools::DESCRIBE_CAPABILITY => false,
        tools::INVOKE_CAPABILITY => {
            let Some(op_id) = input.get("operation_id").and_then(|v| v.as_str()) else {
                return true; // malformed → approve
            };
            match catalog.and_then(|c| c.get(op_id)) {
                Some(op) => policy::is_mutating(&op.method),
                None => true, // unknown op / no catalog → approve
            }
        }
        _ => true,
    }
}

// ── Permission filter (applied to ALL three tools) ───────────────────────────

/// True when `user` may run `op`. Admins short-circuit (mirrors
/// `RequirePermissions`); otherwise the op's required permission must be held.
/// An op with no declared permission (and not denied by policy) is allowed —
/// the real route enforces nothing there either.
pub fn user_may_run(user: &User, groups: &[Group], op: &Operation) -> bool {
    if user.is_admin {
        return true;
    }
    // ALL of them — `RequirePermissions` enforces the whole tuple, so holding
    // one permission of an ALL-of pair is not authorization. Gating on the first
    // alone would offer the model an operation the real route then refuses.
    op.required_permissions
        .iter()
        .all(|perm| check_permission_union(user, groups, perm))
}

/// An op is offered to the model only when it is not policy-denied AND the user
/// is permitted to run it.
fn op_available(user: &User, groups: &[Group], op: &Operation) -> bool {
    !policy::is_denied(op) && user_may_run(user, groups, op)
}

// ── list_capabilities ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct ListArgs {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    tag: Option<String>,
}

// ── search (tokenized, all-terms, relevance-ranked) ──────────────────────────
//
// The shipped matcher lowercased the WHOLE query and did one `.contains(q)` per
// field, so a natural two-word request — the one a model actually sends —
// matched NOTHING: no `operation_id` or `tag` ever contains a space, and no
// summary contains the literal phrase `"create project"`. Live evidence: a real
// session's `list_capabilities{query:"create project"}` returned 0 while
// `"project"` returned 24 and `"create"` returned 21, and the model then flailed.
//
// So: split the query on whitespace and require EVERY term to match at least one
// field (each term may match a different field), then order by relevance so the
// operation the phrase actually describes comes first. Single-term queries are
// unaffected in membership (one term ⇒ the same predicate) and, when every
// candidate scores alike, the `operation_id` tie-break reproduces the old
// alphabetical order exactly.

/// Per-term score for the best field it hits. Higher = more precise signal.
/// `operation_id` is "the stable key the model addresses", so it outranks the
/// noisier prose of `summary`.
const SCORE_ID_SEGMENT: u32 = 8;
const SCORE_ID_SUBSTRING: u32 = 6;
const SCORE_TAG_EXACT: u32 = 4;
const SCORE_SUMMARY_WORD: u32 = 3;
/// Also the score for a TAG substring — the two weakest signals share a value.
const SCORE_SUMMARY_SUBSTRING: u32 = 1;

/// Cap on how many terms one query may contribute. A model-supplied query is
/// untrusted input on a synchronous scoring path over the whole catalog, so it
/// gets a bound rather than an unbounded product.
const MAX_QUERY_TERMS: usize = 16;

/// Shortest term allowed to match as a SUBSTRING **in a multi-term query**.
///
/// A short token substring-matches almost everything (`"a"` is inside 407 of the
/// 446 catalog operation ids), which turns relevance into noise once several
/// terms are combined. Terms below this length may still match EXACTLY — as an id
/// segment, a tag, or a summary word — so `"mcp"` still hits
/// `Project.updateMcpSettings`.
///
/// It is NOT applied to a SINGLE-term query: there, substring matching is the
/// whole point and the design requires "single-term behavior at least as good as
/// today" (`"git"` must still find `Hub.refreshAssistants`, `"key"` the
/// user-key operations).
const MIN_SUBSTRING_TERM_LEN: usize = 4;

/// Closed-class words a person or a model puts in a request but which carry no
/// search signal ("create **a new** project, **please**"). Under the ALL-terms
/// rule one of these would otherwise empty an otherwise-good query.
///
/// Deliberately tiny and closed-class: only words that can never be part of an
/// operation name. Domain words (`new`, `list`, `default`) are NOT here — they
/// are exactly the signal the ranking needs.
const QUERY_STOPWORDS: &[&str] = &[
    "a", "an", "the", "please", "for", "me", "my", "to", "of", "in", "on", "and", "or", "with",
    "can", "you", "i", "it", "is", "are", "do", "does", "that", "this", "there", "would", "could",
    "should", "want", "need", "am", "be",
];

/// Lowercase, punctuation-split, stopword-filtered search terms.
///
/// The CORPUS is normalized (ids split on `.`/`_`/`-`/camelCase, summaries split
/// on any non-alphanumeric), so the QUERY is split the SAME way — otherwise a
/// trailing comma or a hyphen turns a good term into one that matches nothing
/// and, under the ALL-terms rule, ONE dead term empties the whole result.
/// `"create a project, please"` and `"update mcp-settings"` must behave like
/// `"create project"` and `"update mcp settings"`.
///
/// Closed-class words are dropped for the same reason: a model asks in
/// sentences, and "please" / "a" / "for me" carry no signal but would each have
/// to match something.
///
/// Empty when the query is blank (or all stopwords), which the caller treats as
/// "no filter" — the unchanged passthrough behavior.
fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .filter(|t| !QUERY_STOPWORDS.contains(&t.as_str()))
        .take(MAX_QUERY_TERMS)
        .collect()
}

/// Lowercase segments of an operation id: `Project.updateMcpSettings` →
/// `["project", "update", "mcp", "settings"]`. Splits on `.`/`_`/`-` and on
/// camelCase boundaries, so a term like `create` matches the `create` segment of
/// `Project.create` exactly rather than merely as a substring.
fn id_segments(operation_id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in operation_id.chars() {
        if ch == '.' || ch == '_' || ch == '-' {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = ch.is_lowercase() || ch.is_numeric();
        cur.extend(ch.to_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Words of a summary, lowercased, split on anything non-alphanumeric.
fn summary_words(summary: &str) -> Vec<String> {
    summary
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// An operation's searchable text, computed ONCE per operation rather than once
/// per (operation × term) — the scoring loop runs over the whole catalog.
struct OpIndex {
    id_segments: Vec<String>,
    id_lower: String,
    summary_words: Vec<String>,
    summary_lower: String,
    tags_lower: Vec<String>,
}

impl OpIndex {
    fn build(op: &Operation) -> Self {
        Self {
            id_segments: id_segments(&op.operation_id),
            id_lower: op.operation_id.to_lowercase(),
            summary_words: summary_words(&op.summary),
            summary_lower: op.summary.to_lowercase(),
            tags_lower: op.tags.iter().map(|t| t.to_lowercase()).collect(),
        }
    }

    /// Score ONE term. `0` means the term does not match this operation at all.
    ///
    /// `allow_short_substring` is set for a single-term query, where substring
    /// matching is the point (and where a short term cannot dilute a conjunction).
    fn term_score(&self, term: &str, allow_short_substring: bool) -> u32 {
        let substring_ok = allow_short_substring || term.len() >= MIN_SUBSTRING_TERM_LEN;
        if self.id_segments.iter().any(|s| s == term) {
            return SCORE_ID_SEGMENT;
        }
        if substring_ok && self.id_lower.contains(term) {
            return SCORE_ID_SUBSTRING;
        }
        if self.tags_lower.iter().any(|t| t == term) {
            return SCORE_TAG_EXACT;
        }
        if self.summary_words.iter().any(|w| w == term) {
            return SCORE_SUMMARY_WORD;
        }
        if substring_ok && self.summary_lower.contains(term) {
            return SCORE_SUMMARY_SUBSTRING;
        }
        // A tag substring is the weakest signal but was matched by the old
        // whole-phrase predicate, so keep it — "single-term behavior at least as
        // good as today".
        if substring_ok && self.tags_lower.iter().any(|t| t.contains(term)) {
            return SCORE_SUMMARY_SUBSTRING;
        }
        0
    }

    /// How many of this operation's id segments were NOT matched.
    ///
    /// The specificity signal behind the tie-break: for `"delete user"`,
    /// `User.delete` leaves 0 segments unmatched while `LitSearch.deleteUserKey`
    /// leaves 3, so the operation the phrase actually names wins instead of
    /// losing to an alphabetical accident.
    fn unmatched_segments(&self, terms: &[String], allow_short_substring: bool) -> usize {
        self.id_segments
            .iter()
            .filter(|seg| {
                !terms.iter().any(|t| {
                    seg == &t
                        || ((allow_short_substring || t.len() >= MIN_SUBSTRING_TERM_LEN)
                            && seg.contains(t.as_str()))
                })
            })
            .count()
    }
}

/// Relevance of an operation for ALL `terms`, or `None` when any term fails to
/// match — the ALL-terms rule. An empty `terms` slice matches everything at
/// score 0 (the no-query passthrough).
///
/// The single-operation form of [`score_indexed`], kept for the tests that pin
/// the predicate directly; `rank_matching_ops` uses the indexed form so the
/// searchable text is built once per operation per call rather than twice.
#[cfg(test)]
fn op_match_score(op: &Operation, terms: &[String]) -> Option<u32> {
    if terms.is_empty() {
        return Some(0);
    }
    score_indexed(&OpIndex::build(op), terms, terms.len() == 1)
}

/// Drop terms that match NOTHING anywhere in the candidate set.
///
/// The ALL-terms rule is right for terms that are part of the vocabulary; it is
/// useless for one that is not. A model writes `create a new project called
/// "Foo"` — `called` and `foo` appear in no operation id, tag or summary, so
/// under a naive conjunction they would empty a query that otherwise names its
/// operation exactly. Dropping a term with zero catalog-wide matches carries no
/// information loss (nothing could ever have matched it) and cannot degenerate:
/// every term that IS in the vocabulary must still match.
///
/// If EVERY term is absent the query is left as-is, so the caller still reports 0
/// with the retry guidance rather than silently listing the whole catalog.
///
/// Single-term queries are exempt: there is no conjunction to rescue, and
/// dropping the only term would turn "no match" into "everything".
fn retain_known_terms(indexes: &[OpIndex], terms: &[String]) -> Vec<String> {
    if terms.len() < 2 {
        return terms.to_vec();
    }
    let kept: Vec<String> = terms
        .iter()
        .filter(|t| indexes.iter().any(|ix| ix.term_score(t, false) > 0))
        .cloned()
        .collect();
    if kept.is_empty() { terms.to_vec() } else { kept }
}

/// Relevance of an operation for ALL `terms` given its prebuilt index, or `None`
/// when any term fails to match — the ALL-terms rule.
fn score_indexed(index: &OpIndex, terms: &[String], allow_short: bool) -> Option<u32> {
    let mut total = 0u32;
    for term in terms {
        match index.term_score(term, allow_short) {
            0 => return None,
            s => total = total.saturating_add(s),
        }
    }
    Some(total)
}

/// Rank the permitted candidates for `terms` — THE production search pipeline.
///
/// Extracted (mirroring `needs_approval_decision`, extracted so the
/// security-critical decision is unit-testable) so the unit tests exercise the
/// REAL matching + ordering rather than a retyped copy of it.
///
/// 1. Terms absent from the candidates' whole vocabulary are dropped
///    ([`retain_known_terms`]) — a filler noun cannot veto a query.
/// 2. Every REMAINING term must match (the ALL-terms rule).
/// 3. Order: relevance DESC, then FEWEST unmatched id segments (specificity —
///    this is what puts `User.delete` above `LitSearch.deleteUserKey` for
///    `"delete user"`, where a pure alphabetical tie-break handed the model a
///    destructive near-miss), then `operation_id` ASC for determinism. With no
///    query, or when everything ties, that reproduces the previous alphabetical
///    order exactly.
///
/// There is deliberately NO "match ANY term" fallback. One was implemented and
/// removed: a short term substring-matches nearly the whole catalog, so a query
/// with one filler word degenerated into "return 200 arbitrary operations",
/// which is worse for the model than an empty result plus the retry guidance
/// `list_capabilities` now emits.
///
/// Each candidate's searchable text is indexed ONCE per call and reused by both
/// passes; the no-query browse path skips indexing entirely.
fn rank_matching_ops<'a>(
    candidates: impl Iterator<Item = &'a Operation>,
    terms: &[String],
) -> Vec<&'a Operation> {
    let candidates: Vec<&Operation> = candidates.collect();
    if terms.is_empty() {
        // No query: no index needed, and the previous alphabetical order stands.
        let mut all = candidates;
        all.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
        return all;
    }

    let indexes: Vec<OpIndex> = candidates.iter().map(|op| OpIndex::build(op)).collect();
    let terms = retain_known_terms(&indexes, terms);
    let allow_short = terms.len() == 1;

    let mut scored: Vec<(u32, usize, &Operation)> = candidates
        .iter()
        .zip(indexes.iter())
        .filter_map(|(op, index)| {
            score_indexed(index, &terms, allow_short)
                .map(|score| (score, index.unmatched_segments(&terms, allow_short), *op))
        })
        .collect();
    scored.sort_by(|(sa, ua, a), (sb, ub, b)| {
        sb.cmp(sa).then_with(|| ua.cmp(ub)).then_with(|| a.operation_id.cmp(&b.operation_id))
    });
    scored.into_iter().map(|(_, _, op)| op).collect()
}

fn list_capabilities(
    user: &User,
    groups: &[Group],
    catalog: &ControlCatalog,
    args: &Value,
) -> Result<Value, AppError> {
    let args: ListArgs = serde_json::from_value(args.clone()).unwrap_or_default();
    let terms = args.query.as_deref().map(query_terms).unwrap_or_default();
    let tag = args.tag.as_deref();

    // The permission filter runs FIRST and is unconditional: ranking only ever
    // reorders operations the caller is already allowed to run.
    let permitted = catalog
        .iter()
        .filter(|op| op_available(user, groups, op))
        .filter(|op| match &tag {
            Some(t) => op.tags.iter().any(|opt| opt.eq_ignore_ascii_case(t)),
            None => true,
        });
    let mut matched: Vec<&Operation> = rank_matching_ops(permitted, &terms);
    let total = matched.len();
    let truncated = total > MAX_LIST_RESULTS;
    matched.truncate(MAX_LIST_RESULTS);

    let items: Vec<Value> = matched
        .iter()
        .map(|op| {
            json!({
                "operation_id": op.operation_id,
                "method": op.method,
                "summary": op.summary,
                "required_permission": op.required_permission,
                "mutating": policy::is_mutating(&op.method),
            })
        })
        .collect();

    let structured = json!({
        "operations": items,
        "returned": items.len(),
        "total": total,
        "truncated": truncated,
    });
    let mut text = format!(
        "{} operation(s) you can run{}:\n",
        total,
        if truncated {
            format!(" (showing first {MAX_LIST_RESULTS})")
        } else {
            String::new()
        }
    );
    for op in &matched {
        text.push_str(&format!("- {} [{}] — {}\n", op.operation_id, op.method, op.summary));
    }
    // A bare "0 operation(s)" told the model nothing about WHY, and it flailed
    // (a real session repeated the same call, then a `describe_capability` for an
    // operation it had never been shown). Say how the match works so a retry is
    // informed rather than random.
    if total == 0 && (!terms.is_empty() || tag.is_some()) {
        text.push_str(&format!(
            "\nEVERY search term must match an operation's id, tags or summary — one term that \
             matches nothing empties the whole result. Your terms were: [{}]{}. Retry with \
             FEWER, more specific keywords (e.g. \"create project\", \"memory settings\"), \
             drop the `tag` filter, or call list_capabilities with no query to browse.\n",
            terms.join(", "),
            tag.map(|t| format!(", filtered to tag \"{t}\"")).unwrap_or_default()
        ));
    }
    Ok(text_result(text, Some(structured)))
}

// ── describe_capability ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DescribeArgs {
    operation_id: String,
}

fn describe_capability(
    user: &User,
    groups: &[Group],
    catalog: &ControlCatalog,
    args: &Value,
) -> Result<Value, AppError> {
    let args: DescribeArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_PARAMS", format!("describe args: {e}")))?;
    let op = resolve_op(user, groups, catalog, &args.operation_id)?;

    // The catalog holds the schema exactly as the OpenAPI document declares it,
    // which for every named request type is a bare
    // `{"$ref": "#/components/schemas/…"}`. That document is in-process only, so
    // an un-inlined schema tells the model nothing about the fields it must
    // send. Resolve it into a self-contained schema before handing it over.
    let inlined: Option<InlinedSchema> = op
        .request_schema
        .as_ref()
        .map(|s| schema_inline::inline_schema(s, catalog.components()));

    // Parameter schemas carry `$ref`s too (an enum-valued query parameter is
    // `{"$ref": "#/components/schemas/HubCategory"}`), and a query parameter is
    // as much part of the input contract as the body — so they get the same
    // treatment. Caught by the catalog-wide sweep, not by the hand-picked
    // operation.
    let parameters: Vec<Value> = op
        .parameters
        .iter()
        .map(|p| inline_parameter_schema(p, catalog.components()))
        .collect();

    let structured = json!({
        "operation_id": op.operation_id,
        "method": op.method,
        "path_template": op.path_template,
        "required_permission": op.required_permission,
        // EVERY permission the route requires. `required_permission` is the
        // first of these, kept as a single label; an ALL-of operation needs all.
        "required_permissions": op.required_permissions,
        "mutating": policy::is_mutating(&op.method),
        "requires_approval": policy::is_mutating(&op.method),
        "path_params": op.path_params,
        "parameters": parameters,
        "request_schema": inlined.as_ref().map(|i| i.schema.clone()),
        // How the schema is expressed (`inline` — every reference expanded in
        // place — or `defs`, where shared/recursive types live in a sibling
        // `$defs`), and whether any type was genuinely elided for size. Both
        // are `null` when the operation takes no JSON body.
        "schema_form": inlined.as_ref().map(|i| i.form.as_str()),
        "schema_truncated": inlined.as_ref().map(|i| i.truncated),
        "summary": op.summary,
    });
    let parameters = structured["parameters"].as_array().cloned().unwrap_or_default();
    Ok(text_result(
        render_describe_digest(op, &parameters, inlined.as_ref()),
        Some(structured),
    ))
}

// ── describe_capability: the model-facing digest ─────────────────────────────

/// How many levels of nesting the digest walks (root properties are level 1).
/// Beyond this the fields are still in the JSON Schema block below the digest —
/// the digest is a reading aid, the schema is the contract — and the digest says
/// so explicitly rather than trailing off silently.
const DIGEST_MAX_DEPTH: usize = 4;
/// Cap on digest LINES. The depth cap alone does not bound a wide schema, and
/// this text goes into the model's context on a tool it is told to call before
/// every invoke.
const DIGEST_MAX_FIELDS: usize = 200;
/// Cap on the whole describe text. The schema block is `to_string_pretty`, ~2x
/// its compact size, so the schema budget alone does not bound what is emitted
/// here. Deliberately far below `MAX_RESULT_BYTES` (1 MiB): that cap exists to
/// stop a runaway response, this one exists to keep a routine call cheap.
const DIGEST_MAX_TEXT_BYTES: usize = 96 * 1024;
/// Enum options listed inline before eliding the tail.
const DIGEST_MAX_ENUM: usize = 12;
/// Per-field description length in the digest (the full text is in the schema).
const DIGEST_MAX_DESC: usize = 200;

/// Inline the `$ref`s inside ONE OpenAPI parameter object's `schema`, leaving
/// the rest of the parameter (`name` / `in` / `required` / `style`) untouched.
fn inline_parameter_schema(param: &Value, components: &Value) -> Value {
    let Some(obj) = param.as_object() else {
        return param.clone();
    };
    let Some(schema) = obj.get("schema") else {
        return param.clone();
    };
    let mut out = obj.clone();
    out.insert(
        "schema".to_string(),
        schema_inline::inline_schema(schema, components).schema,
    );
    Value::Object(out)
}

/// Render `describe_capability`'s text channel.
///
/// Deliberately NOT `to_string_pretty(&structured)`. The repo convention for a
/// built-in tool (the `web_search` retrofit) is a readable digest in the text
/// channel with typed data in `structuredContent` — and here the digest earns
/// its place twice over: the flattened field list, with each field's type,
/// requiredness, default and enum options, is exactly the material an `ask_user`
/// form is built from.
///
/// The exact JSON Schema is ALWAYS appended, never replaced by the digest:
/// request bodies here nest (an object property that is itself an object, an
/// array of objects, a nullable `anyOf` wrapper), and the digest abbreviates
/// past [`DIGEST_MAX_DEPTH`], so the schema block is what keeps the contract
/// complete.
fn render_describe_digest(
    op: &Operation,
    parameters: &[Value],
    inlined: Option<&InlinedSchema>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} — {} {}\n",
        op.operation_id, op.method, op.path_template
    ));
    if !op.summary.is_empty() {
        out.push_str(&format!("{}\n", op.summary));
    }
    out.push_str(&match op.required_permissions.len() {
        // "(none detected)", not "(none declared)": a handful of routes are
        // genuinely public, but a missing declaration can also mean we could not
        // recover one — the model must not read this as "guaranteed allowed".
        0 => "Required permission: (none detected — this operation may still be refused)\n"
            .to_string(),
        1 => format!("Required permission: {}\n", op.required_permissions[0]),
        _ => format!(
            "Required permissions (ALL of): {}\n",
            op.required_permissions.join(", ")
        ),
    });
    let mutating = policy::is_mutating(&op.method);
    out.push_str(&format!(
        "Requires approval: {}\n",
        if mutating {
            "yes — state-changing, the user must approve before it runs"
        } else {
            "no — read-only"
        }
    ));

    if op.path_params.is_empty() {
        out.push_str("Path parameters: (none)\n");
    } else {
        out.push_str(&format!(
            "Path parameters (all required): {}\n",
            op.path_params.join(", ")
        ));
    }
    let query = render_query_params(parameters);
    out.push_str(&format!(
        "Query parameters: {}\n",
        if query.is_empty() { "(none)".to_string() } else { query }
    ));

    match inlined {
        None => {
            out.push_str("\nRequest body: (none — this operation takes no JSON body)\n");
        }
        Some(i) => {
            let defs = i.schema.get("$defs").and_then(|d| d.as_object());
            let mut fields = Vec::new();
            collect_fields(&i.schema, "", 0, defs, &mut fields);
            out.push_str("\nRequest body fields:\n");
            if fields.is_empty() {
                out.push_str("  (no named properties — see the JSON Schema below)\n");
            } else {
                for f in &fields {
                    out.push_str(&f);
                    out.push('\n');
                }
            }
            if i.truncated {
                out.push_str(
                    "  NOTE: part of this schema was omitted for size; the omitted types are \
                     named in `$defs` below.\n",
                );
            }
            out.push_str("\nJSON Schema (exact — use this to build the body):\n");
            match serde_json::to_string_pretty(&i.schema) {
                Ok(pretty) => out.push_str(&pretty),
                Err(e) => {
                    // Never leave the header asserting an exact contract follows
                    // and then supply nothing.
                    tracing::warn!(error = %e, "control_mcp: could not render the schema block");
                    out.push_str("(unavailable — read `structuredContent.request_schema`)");
                }
            }
            out.push('\n');
        }
    }
    clamp_digest(out)
}

/// Bound the whole describe text. The schema budget measures COMPACT bytes while
/// this channel emits pretty-printed JSON, so without this the one thing the
/// design requires to be bounded — what actually enters the model's context — is
/// the one thing unmeasured. Cuts at a line boundary and says that it did.
fn clamp_digest(text: String) -> String {
    if text.len() <= DIGEST_MAX_TEXT_BYTES {
        return text;
    }
    let cut = text[..DIGEST_MAX_TEXT_BYTES]
        .rfind('\n')
        .unwrap_or(DIGEST_MAX_TEXT_BYTES);
    let mut out = text[..cut].to_string();
    out.push_str(
        "\n… (this description was truncated for size;          read `structuredContent.request_schema` for the exact, complete schema)\n",
    );
    out
}

/// `page (integer, in query)` lines for the operation's declared parameters,
/// skipping the path ones already listed above.
fn render_query_params(parameters: &[Value]) -> String {
    let mut parts = Vec::new();
    for p in parameters {
        if p.get("in").and_then(|v| v.as_str()) != Some("query") {
            continue;
        }
        let Some(name) = p.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let schema = p.get("schema").unwrap_or(&Value::Null);
        let ty = schema_type_label(schema);
        let req = if p.get("required").and_then(|v| v.as_bool()).unwrap_or(false) {
            " REQUIRED"
        } else {
            ""
        };
        let opts = enum_options(schema)
            .map(|o| format!(" one of: {o}"))
            .unwrap_or_default();
        // Same reasoning as for body fields: a bound the model does not see is a
        // 400 it cannot avoid.
        let cons = constraint_label(schema)
            .map(|c| format!(" [{c}]"))
            .unwrap_or_default();
        parts.push(format!("{name} ({ty}){req}{cons}{opts}"));
    }
    parts.join(", ")
}

/// Walk the body schema and push one `- name (type) REQUIRED — description`
/// line per field, RECURSING into nested objects (`parent.child`) and into
/// arrays of objects (`items[].child`).
///
/// Nesting is part of the contract, not noise: an operation whose body carries
/// an object-valued property is unusable if the model only ever sees the
/// top-level key names.
fn collect_fields(
    schema: &Value,
    prefix: &str,
    depth: usize,
    defs: Option<&serde_json::Map<String, Value>>,
    out: &mut Vec<String>,
) {
    if depth >= DIGEST_MAX_DEPTH || out.len() >= DIGEST_MAX_FIELDS {
        // Say so, once, rather than trailing off silently: a model that cannot
        // tell "no more fields" from "not shown" will build an incomplete body.
        let marker = "  … (deeper fields omitted from this summary — see the JSON Schema below)";
        if !out.iter().any(|l| l == marker) {
            out.push(marker.to_string());
        }
        return;
    }
    let resolved = follow_defs(schema, defs);
    let Some(obj) = resolved.as_object() else {
        return;
    };
    let required: Vec<&str> = obj
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let Some(props) = obj.get("properties").and_then(|p| p.as_object()) else {
        return;
    };

    for (name, field) in props {
        let field = follow_defs(field, defs);
        let path = format!("{prefix}{name}");
        let indent = "  ".repeat(depth + 1);
        let mut line = format!(
            "{indent}- {path} ({})",
            schema_type_label(&field)
        );
        if required.contains(&name.as_str()) {
            line.push_str(" REQUIRED");
        }
        if let Some(c) = constraint_label(&field) {
            line.push_str(&format!(" {c}"));
        }
        match field.get("default") {
            // A default that VIOLATES the field's own constraints is worse than
            // no default: it re-supplies the "I may omit this" conclusion the
            // constraint was added to remove, and hands the model an invalid
            // value to send. `CreateProjectRequest.name` is exactly this shape
            // (`default: ""` with `minLength: 1`).
            Some(d) if default_violates_constraints(d, &field) => {
                line.push_str(" (no usable default — a value is required)");
            }
            Some(d) => line.push_str(&format!(" default={d}")),
            None => {}
        }
        if let Some(opts) = enum_options(&field) {
            line.push_str(&format!(" one of: {opts}"));
        }
        if let Some(desc) = field
            .get("description")
            .and_then(|d| d.as_str())
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            line.push_str(&format!(" — {}", truncate_desc(desc)));
        }
        out.push(line);

        // Recurse: a nested object, or an array whose items are objects.
        let inner = unwrap_nullable(&field, defs);
        if inner.get("properties").is_some() {
            collect_fields(&inner, &format!("{path}."), depth + 1, defs, out);
        } else if let Some(items) = inner.get("items") {
            let items = follow_defs(items, defs);
            let items = unwrap_nullable(&items, defs);
            if items.get("properties").is_some() {
                collect_fields(&items, &format!("{path}[]."), depth + 1, defs, out);
            }
        }
    }
}

/// Resolve a `#/$defs/Name` pointer inside the document we just emitted, so a
/// field cut into `$defs` (a cycle, or a budget cut) still shows its fields in
/// the digest rather than reading as an opaque pointer.
fn follow_defs(schema: &Value, defs: Option<&serde_json::Map<String, Value>>) -> Value {
    let Some(name) = schema
        .get("$ref")
        .and_then(|r| r.as_str())
        .and_then(|r| r.strip_prefix("#/$defs/"))
    else {
        return schema.clone();
    };
    defs.and_then(|d| d.get(name))
        .cloned()
        .unwrap_or_else(|| schema.clone())
}

/// `anyOf: [{...}, {"type":"null"}]` is how an optional sub-object is modeled
/// here; return the meaningful branch so its fields are still walked.
fn unwrap_nullable(schema: &Value, defs: Option<&serde_json::Map<String, Value>>) -> Value {
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(variants) = schema.get(key).and_then(|v| v.as_array()) {
            for v in variants {
                let v = follow_defs(v, defs);
                if v.get("type").and_then(|t| t.as_str()) == Some("null") {
                    continue;
                }
                if v.get("properties").is_some() || v.get("items").is_some() {
                    return v;
                }
            }
        }
    }
    schema.clone()
}

/// A short human label for a field's type: `string`, `integer`, `object`,
/// `string[]`, `string|null`, or `enum` when only values are declared.
fn schema_type_label(schema: &Value) -> String {
    if let Some(t) = schema.get("type").and_then(|t| t.as_str()) {
        if t == "array" {
            let inner = schema
                .get("items")
                .map(schema_type_label)
                .unwrap_or_else(|| "any".to_string());
            return format!("{inner}[]");
        }
        return t.to_string();
    }
    if let Some(types) = schema.get("type").and_then(|t| t.as_array()) {
        let parts: Vec<&str> = types.iter().filter_map(|t| t.as_str()).collect();
        if !parts.is_empty() {
            return parts.join("|");
        }
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema.get(key).and_then(|v| v.as_array()) {
            let parts: Vec<String> = variants.iter().map(schema_type_label).collect();
            if !parts.is_empty() {
                return parts.join("|");
            }
        }
    }
    if schema.get("enum").is_some() {
        return "enum".to_string();
    }
    if schema.get("properties").is_some() {
        return "object".to_string();
    }
    if schema.get("$ref").is_some() {
        return "object".to_string();
    }
    "any".to_string()
}

/// Compact constraint hint: `len 1..255`, `1..100`, `format=uuid`.
///
/// This is not decoration. Several ziee request types declare no JSON-Schema
/// `required` array (serde supplies a default) yet constrain the value —
/// `CreateProjectRequest.name` has `default: ""` with `minLength: 1`, so it IS
/// mandatory in practice. Without the constraint the model reads
/// `name (string) default=""` and reasonably concludes it may omit it.
fn constraint_label(schema: &Value) -> Option<String> {
    // `as_f64`, not `as_i64`: schemars emits FLOAT bounds for f64 fields
    // (`minimum: 0.0` on temperature/top_p), which an integer read drops
    // silently — exactly the information loss this label exists to prevent.
    let num = |k: &str| schema.get(k).and_then(|v| v.as_f64());
    let fmt = |x: f64| {
        if x.fract() == 0.0 && x.abs() < 1e15 {
            format!("{}", x as i64)
        } else {
            format!("{x}")
        }
    };
    let mut parts = Vec::new();
    match (num("minLength"), num("maxLength")) {
        (Some(lo), Some(hi)) => parts.push(format!("len {}..{}", fmt(lo), fmt(hi))),
        (Some(lo), None) => parts.push(format!("len {}..", fmt(lo))),
        (None, Some(hi)) => parts.push(format!("len ..{}", fmt(hi))),
        (None, None) => {}
    }
    match (num("minItems"), num("maxItems")) {
        (Some(lo), Some(hi)) => parts.push(format!("{}..{} items", fmt(lo), fmt(hi))),
        (Some(lo), None) => parts.push(format!(">={} items", fmt(lo))),
        (None, Some(hi)) => parts.push(format!("<={} items", fmt(hi))),
        (None, None) => {}
    }
    match (num("minimum"), num("maximum")) {
        (Some(lo), Some(hi)) => parts.push(format!("{}..{}", fmt(lo), fmt(hi))),
        (Some(lo), None) => parts.push(format!(">={}", fmt(lo))),
        (None, Some(hi)) => parts.push(format!("<={}", fmt(hi))),
        (None, None) => {}
    }
    if let Some(p) = schema.get("pattern").and_then(|v| v.as_str()) {
        // The constraint most likely to decide whether the value the model (or
        // an `ask_user` form) produces is accepted at all.
        parts.push(format!("pattern={p}"));
    }
    if let Some(f) = schema.get("format").and_then(|v| v.as_str()) {
        parts.push(format!("format={f}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// True when a declared `default` cannot actually satisfy the field's own
/// constraints — the serde-default-plus-`minLength` shape that makes a field
/// mandatory in practice while the JSON Schema `required` array stays silent.
fn default_violates_constraints(default: &Value, schema: &Value) -> bool {
    match default {
        Value::String(s) => {
            let n = s.chars().count() as i64;
            let too_short = schema
                .get("minLength")
                .and_then(|v| v.as_i64())
                .is_some_and(|min| n < min);
            let bad_enum = schema
                .get("enum")
                .and_then(|e| e.as_array())
                .is_some_and(|vals| !vals.iter().any(|v| v == default));
            too_short || bad_enum
        }
        Value::Array(a) => schema
            .get("minItems")
            .and_then(|v| v.as_i64())
            .is_some_and(|min| (a.len() as i64) < min),
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or_default();
            let below = schema
                .get("minimum")
                .and_then(|v| v.as_f64())
                .is_some_and(|min| x < min);
            let above = schema
                .get("maximum")
                .and_then(|v| v.as_f64())
                .is_some_and(|max| x > max);
            below || above
        }
        _ => false,
    }
}

/// `"a", "b", "c" (+2 more)` for an `enum`, including the array-of-enum shape.
fn enum_options(schema: &Value) -> Option<String> {
    let values = schema
        .get("enum")
        .and_then(|e| e.as_array())
        .or_else(|| schema.get("items").and_then(|i| i.get("enum")).and_then(|e| e.as_array()))?;
    if values.is_empty() {
        return None;
    }
    let shown: Vec<String> = values
        .iter()
        .take(DIGEST_MAX_ENUM)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect();
    let mut s = shown.join(", ");
    if values.len() > DIGEST_MAX_ENUM {
        s.push_str(&format!(" (+{} more)", values.len() - DIGEST_MAX_ENUM));
    }
    Some(s)
}

fn truncate_desc(desc: &str) -> String {
    let one_line = desc.replace(['\n', '\r'], " ");
    if one_line.chars().count() <= DIGEST_MAX_DESC {
        return one_line;
    }
    let cut: String = one_line.chars().take(DIGEST_MAX_DESC).collect();
    format!("{cut}…")
}

// ── invoke_capability ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct InvokeArgs {
    operation_id: String,
    #[serde(default)]
    path_params: std::collections::HashMap<String, String>,
    #[serde(default)]
    query: Option<Value>,
    #[serde(default)]
    body: Option<Value>,
}

/// Copyable literal-JSON examples carried by every `invoke_capability` argument
/// refusal, so the model is shown the shape rather than told about it.
const INVOKE_BODY_EXAMPLE: &str = r#"{"name":"My project","description":"..."}"#;
const INVOKE_QUERY_EXAMPLE: &str = r#"{"page":1,"per_page":50}"#;
const INVOKE_PATH_PARAMS_EXAMPLE: &str = r#"{"id":"3f1c…-uuid"}"#;

/// Decode the JSON-ENCODED object arguments of an `invoke_capability` call.
///
/// Extracted as a pure function so the whole shape distribution can be driven
/// through it directly — the defect this fixes shipped because every existing
/// fixture built these arguments as well-formed objects.
fn decode_invoke_args(args: &Value) -> Result<Value, AppError> {
    let mut out = args.clone();
    crate::common::tool_args::coerce_args_in_place(
        &mut out,
        &[
            crate::common::tool_args::ArgSpec {
                key: "body",
                shape: crate::common::tool_args::ArgShape::Object,
                example: INVOKE_BODY_EXAMPLE,
            },
            crate::common::tool_args::ArgSpec {
                key: "query",
                shape: crate::common::tool_args::ArgShape::Object,
                example: INVOKE_QUERY_EXAMPLE,
            },
            crate::common::tool_args::ArgSpec {
                key: "path_params",
                shape: crate::common::tool_args::ArgShape::Object,
                example: INVOKE_PATH_PARAMS_EXAMPLE,
            },
        ],
    )
    .map_err(|e| AppError::bad_request("INVALID_PARAMS", e.into_message()))?;
    Ok(out)
}

async fn invoke_capability(
    user: &User,
    groups: &[Group],
    catalog: &ControlCatalog,
    headers: &HeaderMap,
    args: &Value,
) -> Result<Value, AppError> {
    // Models routinely JSON-ENCODE a nested object argument one level too many.
    // Decode `body` / `query` / `path_params` BEFORE the typed deserialization
    // below, which would otherwise: hard-fail on `path_params` naming the whole
    // args blob; SILENTLY DROP a stringified `query` (the loopback call then ran
    // with no query params and returned a plausible 200 for the wrong query);
    // and POST a stringified `body` as a JSON string literal, so the real route
    // answered 422 and blamed the wrong layer. Each refusal names what arrived,
    // what is required, and shows a body the model can copy.
    let args_value = decode_invoke_args(args)?;

    let args: InvokeArgs = serde_json::from_value(args_value)
        .map_err(|e| AppError::bad_request("INVALID_PARAMS", format!("invoke args: {e}")))?;
    let op = resolve_op(user, groups, catalog, &args.operation_id)?;

    // Validate the body shape up front (deterministic; nested validation is the
    // real route's job — it returns 400s we relay back). Checked even when the
    // operation declares NO request schema: `validate_body` short-circuits on
    // the schema, so a non-object body used to skip validation entirely there
    // and surfaced as a confusing 422 from the target route instead.
    if let Some(body) = &args.body {
        let schema = op.request_schema.clone().unwrap_or(Value::Null);
        if let Err(msg) = validate_body(&schema, body, catalog.components()) {
            return Err(AppError::bad_request("INVALID_BODY", msg));
        }
    }

    // Substitute + strictly validate path params.
    let path = substitute_path(&op.path_template, &op.path_params, &args.path_params)?;

    let base = CONTROL_BASE_URL
        .get()
        .ok_or_else(|| AppError::internal_error("control base url not initialized"))?;
    let mut url = reqwest::Url::parse(&format!("{base}{path}"))
        .map_err(|e| AppError::internal_with_id(format!("parse loopback url: {e}")))?;

    if let Some(Value::Object(q)) = &args.query {
        let mut pairs = url.query_pairs_mut();
        for (k, v) in q {
            let vs = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            pairs.append_pair(k, &vs);
        }
    }

    let method = reqwest::Method::from_bytes(op.method.as_bytes())
        .map_err(|e| AppError::internal_with_id(format!("parse method: {e}")))?;
    let client = HTTP_CLIENT
        .as_ref()
        .ok_or_else(|| AppError::internal_error("control_mcp loopback client unavailable"))?;
    let mut request = client.request(method, url);

    // Forward the caller's bearer so the real route re-authorizes as this user.
    if let Some(auth_header) = headers.get("authorization").or_else(|| headers.get("Authorization")) {
        request = request.header(reqwest::header::AUTHORIZATION, auth_header);
    }
    // NOTE: we intentionally do NOT forward `x-sync-connection-id`. A control
    // mutation is model-initiated, so the originating device SHOULD receive the
    // resulting sync event (to update its UI) — forwarding the self-suppression
    // header would hide the change from that device (L7).
    if let Some(body) = &args.body {
        request = request.json(body);
    }

    let resp = request
        .send()
        .await
        .map_err(|e| AppError::internal_with_id(format!("loopback dispatch: {e}")))?;

    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::internal_with_id(format!("read loopback response: {e}")))?;

    let (text_body, truncated) = if bytes.len() > MAX_RESULT_BYTES {
        (
            String::from_utf8_lossy(&bytes[..MAX_RESULT_BYTES]).to_string(),
            true,
        )
    } else {
        (String::from_utf8_lossy(&bytes).to_string(), false)
    };
    let parsed: Option<Value> = serde_json::from_str(&text_body).ok();

    let is_error = !status.is_success();
    let structured = json!({
        "operation_id": op.operation_id,
        "status": status.as_u16(),
        "ok": status.is_success(),
        "truncated": truncated,
        "response": parsed.clone().unwrap_or(Value::Null),
    });
    let summary = if is_error {
        format!(
            "{} {} → HTTP {} (error). Response:\n{}",
            op.method, op.path_template, status, text_body
        )
    } else {
        format!(
            "{} {} → HTTP {} (ok). Response:\n{}",
            op.method, op.path_template, status, text_body
        )
    };

    let mut result = text_result(summary, Some(structured));
    if is_error {
        result["isError"] = Value::Bool(true);
    }
    Ok(result)
}

// ── shared helpers ───────────────────────────────────────────────────────────

/// Resolve an operation_id to an [`Operation`] the user may run. A denied op OR
/// one the user lacks permission for returns the SAME "not permitted" error, so
/// the model can't distinguish "forbidden" from "doesn't exist" — no probing.
fn resolve_op<'a>(
    user: &User,
    groups: &[Group],
    catalog: &'a ControlCatalog,
    operation_id: &str,
) -> Result<&'a Operation, AppError> {
    match catalog.get(operation_id) {
        Some(op) if op_available(user, groups, op) => Ok(op),
        _ => Err(AppError::forbidden(
            "NOT_PERMITTED",
            format!("operation '{operation_id}' is not available to you"),
        )),
    }
}

/// Substitute `{name}` path params. Each value must be present and contain only
/// URL-path-safe characters (alphanumerics + `-._~`), which blocks path
/// traversal (`..`, `/`) and host injection — the model cannot redirect the
/// loopback call off its intended route.
fn substitute_path(
    template: &str,
    expected: &[String],
    provided: &std::collections::HashMap<String, String>,
) -> Result<String, AppError> {
    let mut path = template.to_string();
    for name in expected {
        let value = provided.get(name).ok_or_else(|| {
            AppError::bad_request(
                "MISSING_PATH_PARAM",
                format!("path parameter '{name}' is required"),
            )
        })?;
        // Reject empty, disallowed chars, AND the dot-segments `.`/`..` — the
        // latter contain no `/` (so they pass the char check) but WHATWG URL
        // normalization collapses them, which would dispatch to a DIFFERENT
        // route than the one authorized/denylist-checked/approved (H1).
        if value.is_empty() || value == "." || value == ".." || !value.chars().all(is_path_safe) {
            return Err(AppError::bad_request(
                "INVALID_PATH_PARAM",
                format!("path parameter '{name}' is empty or contains disallowed characters"),
            ));
        }
        path = path.replace(&format!("{{{name}}}"), value);
    }
    Ok(path)
}

fn is_path_safe(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~')
}

/// Deterministic, shallow body validation: the top-level object's `required`
/// fields must be present, and — when `additionalProperties: false` — no unknown
/// keys are allowed. Deep/nested validation is delegated to the real route
/// (which returns a 400 we relay back), so we never falsely reject on an
/// OpenAPI/JSON-Schema dialect quirk.
fn validate_body(schema: &Value, body: &Value, components: &Value) -> Result<(), String> {
    // A scalar body is never a valid JSON request body for ANY route, so this
    // check runs BEFORE the schema short-circuits below. It used to sit after
    // them, so an operation with no (or a non-object) request schema skipped
    // validation entirely and a JSON-ENCODED body was POSTed as a string
    // literal — the target route then answered 422 and the model was blamed by
    // the wrong layer. Arrays are deliberately still allowed through here: a
    // route taking `Json<Vec<T>>` legitimately receives one, and the
    // object-typed schema branch below rejects an array when the schema does
    // say `object`.
    if matches!(body, Value::String(_) | Value::Number(_) | Value::Bool(_)) {
        return Err(format!(
            "`body` arrived as {}, but a JSON object is required. Send `body` as a JSON \
             object, not a JSON-encoded string. Example: {INVOKE_BODY_EXAMPLE}",
            match body {
                Value::String(_) => "a string",
                Value::Number(_) => "a number",
                _ => "a boolean",
            }
        ));
    }

    let resolved = resolve_schema_ref(schema, components);
    let Some(obj) = resolved.as_object() else {
        return Ok(());
    };
    // Only validate object bodies.
    if obj.get("type").and_then(|t| t.as_str()) != Some("object") {
        return Ok(());
    }
    let body_obj = match body {
        Value::Object(m) => m,
        Value::Null => return Ok(()),
        _ => {
            return Err(format!(
                "`body` arrived as an array, but this operation requires a JSON object. \
                 Send `body` as a JSON object. Example: {INVOKE_BODY_EXAMPLE}"
            ));
        }
    };

    if let Some(required) = obj.get("required").and_then(|r| r.as_array()) {
        for field in required.iter().filter_map(|f| f.as_str()) {
            if !body_obj.contains_key(field) {
                return Err(format!("missing required field '{field}'"));
            }
        }
    }
    if obj.get("additionalProperties") != Some(&Value::Bool(false)) {
        return Ok(());
    }
    if let Some(props) = obj.get("properties").and_then(|p| p.as_object())
        && let Some(key) = body_obj.keys().find(|k| !props.contains_key(*k))
    {
        return Err(format!("unknown field '{key}'"));
    }
    Ok(())
}

fn text_result(text: impl Into<String>, structured: Option<Value>) -> Value {
    let mut obj = json!({ "content": [{ "type": "text", "text": text.into() }] });
    if let Some(s) = structured {
        obj["structuredContent"] = s;
    }
    obj
}

fn ok_response(id: Option<Value>, result: Value) -> Response {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }),
    )
        .into_response()
}

fn error_response(id: Option<Value>, http: StatusCode, err: JsonRpcError) -> Response {
    (
        http,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema_obj() -> Value {
        json!({
            "type": "object",
            "required": ["username"],
            "additionalProperties": false,
            "properties": {
                "username": { "type": "string" },
                "email": { "type": "string" }
            }
        })
    }

    #[test]
    fn validate_body_accepts_valid() {
        let c = json!({});
        assert!(validate_body(&schema_obj(), &json!({"username": "a"}), &c).is_ok());
        assert!(validate_body(&schema_obj(), &json!({"username": "a", "email": "x"}), &c).is_ok());
    }

    #[test]
    fn validate_body_rejects_missing_required() {
        let c = json!({});
        let err = validate_body(&schema_obj(), &json!({"email": "x"}), &c).unwrap_err();
        assert!(err.contains("username"));
    }

    #[test]
    fn validate_body_rejects_unknown_field() {
        let c = json!({});
        let err = validate_body(&schema_obj(), &json!({"username": "a", "role": "admin"}), &c)
            .unwrap_err();
        assert!(err.contains("role"));
    }

    #[test]
    fn validate_body_resolves_ref() {
        let components = json!({ "schemas": { "UserCreate": schema_obj() } });
        let schema = json!({ "$ref": "#/components/schemas/UserCreate" });
        assert!(validate_body(&schema, &json!({"username": "a"}), &components).is_ok());
        assert!(
            validate_body(&schema, &json!({"email": "x"}), &components)
                .unwrap_err()
                .contains("username")
        );
        // TEST-12 — the duplicate private resolver was deleted in favour of
        // `schema_inline::resolve_schema_ref`; the pre-existing FAIL-OPEN
        // behaviour on an unresolvable ref must be preserved, because
        // `validate_body` relies on it to skip local validation and let the real
        // route decide (rather than falsely rejecting a valid body).
        let dangling = json!({ "$ref": "#/components/schemas/DoesNotExist" });
        assert!(
            validate_body(&dangling, &json!({ "anything": 1 }), &components).is_ok(),
            "an unresolvable body schema must fail open, not reject"
        );
    }

    #[test]
    fn substitute_path_replaces_and_validates() {
        let expected = vec!["user_id".to_string()];
        let mut provided = std::collections::HashMap::new();
        provided.insert("user_id".to_string(), "abc-123".to_string());
        assert_eq!(
            substitute_path("/api/users/{user_id}", &expected, &provided).unwrap(),
            "/api/users/abc-123"
        );
    }

    #[test]
    fn substitute_path_rejects_traversal_and_missing() {
        let expected = vec!["id".to_string()];
        let mut bad = std::collections::HashMap::new();
        bad.insert("id".to_string(), "../secret".to_string());
        assert!(substitute_path("/api/x/{id}", &expected, &bad).is_err());

        let empty = std::collections::HashMap::new();
        assert!(substitute_path("/api/x/{id}", &expected, &empty).is_err());

        let mut slash = std::collections::HashMap::new();
        slash.insert("id".to_string(), "a/b".to_string());
        assert!(substitute_path("/api/x/{id}", &expected, &slash).is_err());

        // Bare dot-segments contain no `/` but WHATWG URL parsing collapses them
        // to a DIFFERENT route — must be rejected (H1).
        for bad in [".", ".."] {
            let mut m = std::collections::HashMap::new();
            m.insert("id".to_string(), bad.to_string());
            assert!(
                substitute_path("/api/projects/{id}/files", &expected, &m).is_err(),
                "path param '{bad}' must be rejected"
            );
        }
    }

    #[test]
    fn is_path_safe_blocks_dangerous_chars() {
        assert!("abc-123_x.y~z".chars().all(is_path_safe));
        assert!(!is_path_safe('/'));
        assert!(!is_path_safe('?'));
        assert!(!is_path_safe('@'));
        assert!(!is_path_safe(' '));
    }

    fn approval_fixture() -> catalog::ControlCatalog {
        catalog::build_catalog(&json!({
            "paths": {
                "/api/users": {
                    "post": { "operationId": "User.create", "summary": "" },
                    "get": { "operationId": "User.list", "summary": "" }
                }
            }
        }))
    }

    #[test]
    fn reads_never_need_approval() {
        let cat = approval_fixture();
        assert!(!needs_approval_decision(tools::LIST_CAPABILITIES, &json!({}), Some(&cat)));
        assert!(!needs_approval_decision(tools::DESCRIBE_CAPABILITY, &json!({}), Some(&cat)));
        // invoke of a GET op → read-only, no approval.
        assert!(!needs_approval_decision(
            tools::INVOKE_CAPABILITY,
            &json!({ "operation_id": "User.list" }),
            Some(&cat)
        ));
    }

    #[test]
    fn mutating_invoke_always_needs_approval() {
        let cat = approval_fixture();
        assert!(needs_approval_decision(
            tools::INVOKE_CAPABILITY,
            &json!({ "operation_id": "User.create" }),
            Some(&cat)
        ));
    }

    #[test]
    fn unknown_or_malformed_fails_safe_to_approval() {
        let cat = approval_fixture();
        // Unknown op.
        assert!(needs_approval_decision(
            tools::INVOKE_CAPABILITY,
            &json!({ "operation_id": "Nope.gone" }),
            Some(&cat)
        ));
        // Missing operation_id.
        assert!(needs_approval_decision(tools::INVOKE_CAPABILITY, &json!({}), Some(&cat)));
        // No catalog at all.
        assert!(needs_approval_decision(
            tools::INVOKE_CAPABILITY,
            &json!({ "operation_id": "User.list" }),
            None
        ));
        // Unknown tool.
        assert!(needs_approval_decision("mystery", &json!({}), Some(&cat)));
    }

    // ── search matcher (TEST-1 / TEST-2 / TEST-3) ────────────────────────────

    /// A slice of the REAL catalog shape — operation ids, summaries and tags
    /// copied from the committed `openapi.json` — so the ranking assertions are
    /// about real data, not about a fixture tuned to pass.
    fn search_fixture() -> catalog::ControlCatalog {
        catalog::build_catalog(&json!({
            "paths": {
                "/api/projects": {
                    "post": {
                        "operationId": "Project.create",
                        "summary": "Create a new chat project",
                        "tags": ["Projects"]
                    },
                    "get": {
                        "operationId": "Project.list",
                        "summary": "List the caller's projects",
                        "tags": ["Projects"]
                    }
                },
                "/api/projects/{id}": {
                    "put": {
                        "operationId": "Project.update",
                        "summary": "Update project",
                        "tags": ["Projects"]
                    },
                    "delete": {
                        "operationId": "Project.delete",
                        "summary": "Delete project",
                        "tags": ["Projects"]
                    }
                },
                "/api/projects/{id}/duplicate": {
                    "post": {
                        "operationId": "Project.duplicate",
                        "summary": "Duplicate a project",
                        "tags": ["Projects"]
                    }
                },
                "/api/projects/{id}/mcp-settings": {
                    "put": {
                        "operationId": "Project.updateMcpSettings",
                        "summary": "Update project MCP defaults",
                        "tags": ["Projects"]
                    }
                },
                "/api/assistants": {
                    "post": {
                        "operationId": "Assistant.create",
                        "summary": "Create a new user assistant",
                        "tags": ["Assistants"]
                    }
                },
                "/api/workflows": {
                    "post": {
                        "operationId": "Workflow.create",
                        "summary": "Create a user-scope workflow from a WorkflowDef",
                        "tags": ["Workflows"]
                    }
                }
            }
        }))
    }

    /// Rank the fixture by `query` through the PRODUCTION pipeline
    /// (`rank_matching_ops`) — never a retyped copy of it, so a change to the
    /// real matcher or comparator turns these tests red.
    fn ranked(cat: &catalog::ControlCatalog, query: &str) -> Vec<String> {
        let terms = query_terms(query);
        rank_matching_ops(cat.iter(), &terms)
            .into_iter()
            .map(|op| op.operation_id.clone())
            .collect()
    }

    /// The SHIPPED matcher, reproduced verbatim, so the regression it caused is
    /// encoded in the suite rather than described in a comment.
    fn legacy_whole_phrase_match(op: &Operation, query: &str) -> bool {
        let q = query.to_lowercase();
        op.operation_id.to_lowercase().contains(&q)
            || op.summary.to_lowercase().contains(&q)
            || op.tags.iter().any(|t| t.to_lowercase().contains(&q))
    }

    /// TEST-1 — the EXACT live-session query.
    ///
    /// A real user asked "create a new project please"; the model sent
    /// `list_capabilities{query:"create project"}` and got **0 results**. The
    /// multi-word query must now match, and `Project.create` must rank FIRST.
    #[test]
    fn create_project_query_matches_and_ranks_project_create_first() {
        let cat = search_fixture();

        // The bug, encoded: the shipped whole-phrase matcher finds NOTHING.
        assert_eq!(
            cat.iter().filter(|op| legacy_whole_phrase_match(op, "create project")).count(),
            0,
            "precondition: the shipped whole-phrase matcher is what returned 0 results"
        );

        let hits = ranked(&cat, "create project");
        assert!(!hits.is_empty(), "'create project' must match at least one operation");
        assert_eq!(
            hits.first().map(String::as_str),
            Some("Project.create"),
            "'create project' must rank Project.create first, got {hits:?}"
        );
    }

    /// TEST-2 — ALL terms must match, and terms may match DIFFERENT fields.
    #[test]
    fn all_terms_must_match_possibly_via_different_fields() {
        let cat = search_fixture();

        let hits = ranked(&cat, "create project");
        assert!(
            !hits.iter().any(|id| id == "Project.list"),
            "Project.list matches only 'project', so it must be excluded: {hits:?}"
        );
        assert!(
            !hits.iter().any(|id| id == "Assistant.create"),
            "Assistant.create matches only 'create', so it must be excluded: {hits:?}"
        );

        // 'assistants' hits only the TAG, 'create' hits only the id segment —
        // one term per field, and the op still matches.
        let hits = ranked(&cat, "assistants create");
        assert_eq!(hits, vec!["Assistant.create".to_string()], "got {hits:?}");

        // The ALL-terms rule binds every term that IS in the vocabulary.
        assert_eq!(
            ranked(&cat, "create assistant"),
            vec!["Assistant.create".to_string()],
            "a known term must still narrow the result"
        );
    }

    /// TEST-3 — single-term behavior is at least as good as today, plus the
    /// empty-query passthrough.
    #[test]
    fn single_term_parity_case_insensitivity_and_empty_query() {
        let cat = search_fixture();

        // Includes SHORT terms: `MIN_SUBSTRING_TERM_LEN` must not apply to a
        // single-term query, or `"mcp"`/`"set"`-style lookups silently regress
        // against the design's "single-term behavior at least as good as today".
        for term in ["project", "create", "PROJECT", "Create", "workflow", "mcp", "set", "up"] {
            let legacy: std::collections::BTreeSet<String> = cat
                .iter()
                .filter(|op| legacy_whole_phrase_match(op, term))
                .map(|op| op.operation_id.clone())
                .collect();
            let now: std::collections::BTreeSet<String> = ranked(&cat, term).into_iter().collect();
            assert!(
                legacy.is_subset(&now),
                "single-term '{term}' regressed: legacy matched {legacy:?}, now {now:?}"
            );
        }

        // Case-insensitive on the multi-word path too.
        assert_eq!(
            ranked(&cat, "CREATE Project").first().map(String::as_str),
            Some("Project.create")
        );

        // Blank query ⇒ no filter (every op), in the previous alphabetical order.
        let all: Vec<String> = ranked(&cat, "");
        assert_eq!(all.len(), cat.len(), "an empty query must not filter anything");
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(all, sorted, "with no query the order must stay operation_id ASC");
        assert_eq!(ranked(&cat, "   ").len(), cat.len(), "whitespace-only is also no filter");
    }

    /// Punctuation and politeness must not empty a good query.
    ///
    /// The corpus is normalized (ids and summaries are split on punctuation), so
    /// an un-normalized query term like `"project,"` would match NOTHING and —
    /// under the ALL-terms rule — take the whole result set down with it. That is
    /// the same class of failure the fix exists to end, just one keystroke away.
    #[test]
    fn punctuation_in_a_query_does_not_empty_the_result() {
        let cat = search_fixture();
        for q in [
            "create a project, please",
            "create project!",
            "  create   project  ",
            "create/project",
        ] {
            let hits = ranked(&cat, q);
            assert_eq!(
                hits.first().map(String::as_str),
                Some("Project.create"),
                "query {q:?} must still rank Project.create first; got {hits:?}"
            );
        }
    }

    /// A natural sentence must survive its filler words.
    ///
    /// A model asks "please create a new project for me", not keyword soup.
    /// Punctuation and closed-class words are stripped query-side so the ALL-terms
    /// rule sees only the real signal — without that, one "please" empties the
    /// result exactly as the shipped whole-phrase matcher did.
    #[test]
    fn a_natural_sentence_still_finds_the_operation() {
        let cat = search_fixture();
        for q in [
            "please create a new project for me",
            "can you create a project?",
            "I want to create a project",
        ] {
            let hits = ranked(&cat, q);
            assert_eq!(
                hits.first().map(String::as_str),
                Some("Project.create"),
                "query {q:?} must still rank Project.create first; got {hits:?}"
            );
        }
    }

    /// A short term is EXACT-only once the query has more than one term.
    ///
    /// `"a"` is a substring of nearly every operation id, so allowing short
    /// substrings inside a conjunction turns relevance into noise. Short terms
    /// still match EXACTLY (segment / tag / summary word), and a SINGLE-term
    /// query keeps full substring behavior (design: "single-term behavior at
    /// least as good as today").
    #[test]
    fn short_terms_are_exact_only_in_a_multi_term_query() {
        let cat = search_fixture();

        // Short but an exact id SEGMENT — matches in both shapes.
        assert_eq!(ranked(&cat, "mcp"), vec!["Project.updateMcpSettings".to_string()]);
        assert_eq!(
            ranked(&cat, "mcp settings"),
            vec!["Project.updateMcpSettings".to_string()],
            "an exact short segment still matches inside a conjunction"
        );

        // Single term: substring behavior retained — "cre" is inside every
        // `*.create` id, and the legacy matcher found them.
        let single = ranked(&cat, "cre");
        assert!(
            single.iter().any(|id| id.ends_with(".create")),
            "a single short term must keep substring behavior; got {single:?}"
        );

        // Multi-term: the same short token cannot act as a substring FILTER —
        // it is exact-matchable nowhere, so it is dropped as vocabulary-absent
        // and the query behaves like its remaining term.
        assert_eq!(
            ranked(&cat, "cre project"),
            ranked(&cat, "project"),
            "a short non-exact term must not silently filter a multi-term query"
        );
    }

    /// A query in which NOTHING is known still returns nothing.
    ///
    /// Dropping vocabulary-absent terms must never degenerate into "list the
    /// catalog": if no term survives, the model gets zero results plus the retry
    /// guidance, not 200 arbitrary operations.
    #[test]
    fn a_query_with_no_known_terms_returns_nothing() {
        let cat = search_fixture();
        assert!(ranked(&cat, "zzzznotathing").is_empty());
        assert!(ranked(&cat, "zzzznotathing qqqnope").is_empty());
    }

    /// Segmentation is what makes an exact word beat a mere substring.
    /// The operation the phrase NAMES must beat an alphabetically-luckier
    /// near-miss.
    ///
    /// Ranking by score alone left ties broken by `operation_id` ASC, which on
    /// the real catalog put `LitSearch.deleteUserKey` above `User.delete` for
    /// `"delete user"` — a destructive near-miss offered first. Fewest UNMATCHED
    /// id segments is the specificity signal that fixes it.
    #[test]
    fn the_named_operation_beats_an_alphabetically_luckier_near_miss() {
        let cat = catalog::build_catalog(&json!({
            "paths": {
                "/api/users/{id}": {
                    "delete": { "operationId": "User.delete", "summary": "Delete a user", "tags": ["Users"] }
                },
                "/api/lit-search/user-key": {
                    "delete": {
                        "operationId": "LitSearch.deleteUserKey",
                        "summary": "Delete the caller's key",
                        "tags": ["LitSearch"]
                    }
                },
                "/api/web-search/user-key": {
                    "delete": {
                        "operationId": "WebSearch.deleteUserKey",
                        "summary": "Delete the caller's key",
                        "tags": ["WebSearch"]
                    }
                }
            }
        }));
        let hits = ranked(&cat, "delete user");
        assert_eq!(
            hits.first().map(String::as_str),
            Some("User.delete"),
            "the operation the phrase names must rank first; got {hits:?}"
        );
    }

    /// A term that is in NO operation's vocabulary must not veto the query.
    ///
    /// A model writes `create a new project called "Foo"`. `called` and `foo`
    /// appear nowhere in the catalog, so a naive conjunction would return zero
    /// for a request that names its operation exactly. Dropping a zero-match term
    /// loses no information — and every term that IS in the vocabulary still has
    /// to match, so this cannot degenerate into an any-term search.
    #[test]
    fn a_term_absent_from_the_whole_catalog_does_not_veto_the_query() {
        let cat = search_fixture();

        for q in [
            "create a new project called Foo",
            "create a project named Zaphod",
        ] {
            let hits = ranked(&cat, q);
            assert_eq!(
                hits.first().map(String::as_str),
                Some("Project.create"),
                "query {q:?} must still rank Project.create first; got {hits:?}"
            );
        }

        // A term that IS in the vocabulary still narrows: `assistant` is real, so
        // it must exclude the project operations rather than being dropped.
        let hits = ranked(&cat, "create assistant zzzznotathing");
        assert_eq!(hits, vec!["Assistant.create".to_string()], "got {hits:?}");

        // If NOTHING in the query is known, the result is still empty (never the
        // whole catalog) — the model gets the retry guidance instead.
        assert!(ranked(&cat, "zzzznotathing qqqnope").is_empty());
    }

    /// Segmentation is what makes an exact word beat a mere substring.
    #[test]
    fn id_segments_splits_on_punctuation_and_camel_case() {
        assert_eq!(id_segments("Project.create"), vec!["project", "create"]);
        assert_eq!(
            id_segments("Project.updateMcpSettings"),
            vec!["project", "update", "mcp", "settings"]
        );
        assert_eq!(id_segments("Hub.createWorkflowFromHub"), vec![
            "hub",
            "create",
            "workflow",
            "from",
            "hub"
        ]);
    }

    // ── describe_capability digest (ITEM-4 / INV-6) ───────────────────────────

    /// A body with a nested object, an array-of-objects, a nullable sub-object,
    /// an enum and a default — the shapes a real ziee request body actually has.
    fn nested_body_schema() -> Value {
        json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string", "description": "Project name" },
                "visibility": { "type": "string", "enum": ["private", "team"], "default": "private" },
                "settings": {
                    "type": "object",
                    "properties": {
                        "loop_limit": { "type": "integer", "default": 10, "description": "Max loops" },
                        "quiet": { "type": "boolean" }
                    }
                },
                "members": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["user_id"],
                        "properties": { "user_id": { "type": "string" }, "role": { "type": "string" } }
                    }
                },
                "owner": {
                    "anyOf": [
                        { "type": "object", "properties": { "email": { "type": "string" } } },
                        { "type": "null" }
                    ]
                }
            }
        })
    }

    fn describe_op(schema: Option<Value>) -> Operation {
        Operation {
            operation_id: "Project.create".into(),
            method: "POST".into(),
            path_template: "/api/projects".into(),
            tags: vec!["Projects".into()],
            summary: "Create a personal chat project".into(),
            required_permission: Some("projects::create".into()),
            required_permissions: vec!["projects::create".into()],
            path_params: vec![],
            request_schema: schema,
            json_body: true,
            has_secret_field: false,
            parameters: vec![json!({
                "in": "query", "name": "per_page", "required": false,
                "schema": { "type": "integer" }
            })],
        }
    }

    /// The operation's parameters as `describe_capability` passes them (already
    /// ref-inlined; the fixture's are ref-free).
    fn inlined_params() -> Vec<Value> {
        describe_op(None)
            .parameters
            .iter()
            .map(|p| inline_parameter_schema(p, &json!({})))
            .collect()
    }

    fn digest_of(schema: Value) -> String {
        let inlined = schema_inline::inline_schema(&schema, &json!({}));
        render_describe_digest(&describe_op(Some(schema)), &inlined_params(), Some(&inlined))
    }

    /// TEST-15 — each top-level field is rendered with its type, requiredness,
    /// default, enum options and description; the header carries the permission
    /// and the approval requirement.
    #[test]
    fn digest_renders_field_type_required_default_enum_and_description() {
        let d = digest_of(nested_body_schema());
        assert!(d.contains("Project.create — POST /api/projects"), "{d}");
        assert!(d.contains("Required permission: projects::create"), "{d}");
        assert!(d.contains("Requires approval: yes"), "{d}");
        assert!(d.contains("per_page (integer)"), "query params: {d}");
        assert!(d.contains("- name (string) REQUIRED — Project name"), "{d}");
        assert!(
            d.contains("- visibility (string) default=\"private\" one of: private, team"),
            "{d}"
        );
        // A non-required field carries no REQUIRED marker.
        let visibility_line = d
            .lines()
            .find(|l| l.contains("- visibility "))
            .expect("visibility line");
        assert!(!visibility_line.contains("REQUIRED"), "{visibility_line}");
    }

    /// TEST-16 (acceptance, INV-6) — nesting is part of the contract. The digest
    /// must name the INNER fields of a nested object, of an array's items, and of
    /// a nullable `anyOf` sub-object — a top-level-keys-only digest fails here.
    #[test]
    fn acceptance_inv6_digest_names_nested_and_array_item_fields() {
        let d = digest_of(nested_body_schema());
        assert!(d.contains("settings.loop_limit"), "nested object field: {d}");
        assert!(d.contains("settings.quiet"), "nested object field: {d}");
        assert!(d.contains("members[].user_id"), "array item field: {d}");
        assert!(d.contains("members[].role"), "array item field: {d}");
        assert!(d.contains("owner.email"), "nullable sub-object field: {d}");
        // Nested requiredness survives the walk.
        assert!(d.contains("members[].user_id (string) REQUIRED"), "{d}");
        // And the nested default is carried, since that is what pre-fills a form.
        assert!(d.contains("settings.loop_limit (integer) default=10"), "{d}");
    }

    /// TEST-19 (acceptance, INV-6) — the digest never REPLACES the schema. The
    /// exact JSON Schema block is always emitted, and round-tripping it out of
    /// the text re-parses to exactly the schema in `structuredContent`.
    #[test]
    fn acceptance_inv6_exact_json_schema_is_always_emitted_alongside_the_digest() {
        let schema = nested_body_schema();
        let inlined = schema_inline::inline_schema(&schema, &json!({}));
        let d = render_describe_digest(&describe_op(Some(schema)), &inlined_params(), Some(&inlined));

        let marker = "JSON Schema (exact — use this to build the body):\n";
        let idx = d.find(marker).expect(&format!("schema block must be present: {d}"));
        let block = &d[idx + marker.len()..];
        let parsed: Value = serde_json::from_str(block.trim())
            .unwrap_or_else(|e| panic!("schema block must re-parse ({e}): {block}"));
        assert_eq!(
            parsed, inlined.schema,
            "the emitted block must be the SAME schema structuredContent carries"
        );
        // The digest is above it, not instead of it.
        assert!(d[..idx].contains("Request body fields:"), "{d}");
    }

    /// A `$ref` cut into `$defs` (a cycle, or a budget cut) still shows its
    /// fields in the digest instead of reading as an opaque pointer.
    #[test]
    fn digest_follows_defs_pointers() {
        let components = json!({ "schemas": {
            "Node": { "type": "object", "properties": {
                "label": { "type": "string" },
                "child": { "$ref": "#/components/schemas/Node" }
            }}
        }});
        let schema = json!({ "$ref": "#/components/schemas/Node" });
        let inlined = schema_inline::inline_schema(&schema, &components);
        let d = render_describe_digest(&describe_op(Some(schema)), &inlined_params(), Some(&inlined));
        assert!(d.contains("- label (string)"), "{d}");
        // The recursive edge resolves through $defs rather than dead-ending.
        assert!(d.contains("child.label"), "$defs pointer must be followed: {d}");
    }

    /// An operation with no JSON body says so plainly rather than emitting an
    /// empty field list or a `null` schema block.
    #[test]
    fn digest_states_when_there_is_no_request_body() {
        let d = render_describe_digest(&describe_op(None), &inlined_params(), None);
        assert!(d.contains("Request body: (none"), "{d}");
        assert!(!d.contains("JSON Schema"), "{d}");
    }


    // ── stringified object arguments (the ask_user twin) ─────────────────────

    /// `invoke_capability`'s three object arguments each decode when the model
    /// JSON-ENCODES them — the same mistake that broke `ask_user`.
    ///
    /// `query` is the worst of the three: it used to fail the
    /// `if let Some(Value::Object(q))` match and be **silently dropped**, so the
    /// loopback call ran with NO query params and returned a plausible 200 for
    /// the wrong query. Nothing anywhere reported a problem. (TEST-22, INV-1)
    #[test]
    fn invoke_args_decode_stringified_body_query_and_path_params() {
        let decoded = decode_invoke_args(&json!({
            "operation_id": "Project.create",
            "body": r#"{"name":"My project"}"#,
            "query": r#"{"page":1}"#,
            "path_params": r#"{"id":"abc"}"#,
        }))
        .expect("stringified object arguments must decode");

        assert_eq!(decoded["body"], json!({ "name": "My project" }));
        assert_eq!(
            decoded["query"],
            json!({ "page": 1 }),
            "a stringified query must reach the URL, not be silently dropped"
        );
        assert_eq!(decoded["path_params"], json!({ "id": "abc" }));
        assert_eq!(
            decoded["operation_id"],
            json!("Project.create"),
            "scalar arguments must never be reparsed"
        );

        // A well-formed call is untouched (no regression), and the typed
        // deserialization still succeeds afterwards.
        let well_formed = json!({
            "operation_id": "Project.create",
            "body": { "name": "My project" },
        });
        assert_eq!(decode_invoke_args(&well_formed).unwrap(), well_formed);
        let parsed: InvokeArgs =
            serde_json::from_value(decode_invoke_args(&well_formed).unwrap()).unwrap();
        assert_eq!(parsed.operation_id, "Project.create");
    }

    /// A `body` that cannot be an object is refused with feedback the model can
    /// act on — what it sent, what is required, and a body it can copy — not
    /// with serde's "invalid type" naming the whole args blob. Asserts the TEXT.
    /// (TEST-23, INV-5)
    #[test]
    fn invoke_args_refusals_tell_the_model_how_to_fix_the_call() {
        for bad in [json!("not json {"), json!("[1,2]"), json!(7)] {
            let err = decode_invoke_args(&json!({ "operation_id": "X", "body": bad.clone() }))
                .expect_err("a non-object body must be refused");
            let msg = format!("{err}");
            assert!(msg.contains("body"), "must name the argument: {msg}");
            assert!(msg.contains("JSON object"), "must say what is expected: {msg}");
            assert!(
                msg.contains(INVOKE_BODY_EXAMPLE),
                "must carry a copyable example: {msg}"
            );
        }
    }

    /// A scalar body is now rejected even when the operation declares NO request
    /// schema. `validate_body` short-circuited on the schema first, so that case
    /// skipped validation entirely and a JSON-encoded body was POSTed as a
    /// string literal — the target route answered 422 and the model was blamed
    /// by the wrong layer. Arrays stay allowed (a `Json<Vec<T>>` route takes
    /// one) unless the schema says `object`. (TEST-25, ITEM-10)
    #[test]
    fn validate_body_rejects_a_scalar_body_even_without_a_schema() {
        let c = json!({});
        let err = validate_body(&Value::Null, &json!("{\"name\":\"x\"}"), &c).unwrap_err();
        assert!(err.contains("JSON object"), "got: {err}");
        assert!(err.contains(INVOKE_BODY_EXAMPLE), "must show a body to copy: {err}");

        // No regression: a well-formed object body against a schema-less
        // operation still passes, and so does an array.
        assert!(validate_body(&Value::Null, &json!({ "name": "x" }), &c).is_ok());
        assert!(validate_body(&Value::Null, &json!([1, 2]), &c).is_ok());
        assert!(validate_body(&Value::Null, &Value::Null, &c).is_ok());
        // …but an array IS rejected when the schema says object.
        assert!(validate_body(&schema_obj(), &json!([1, 2]), &c).is_err());
    }

    /// The shared model-supplied-argument conformance battery, applied to
    /// `invoke_capability.body` — the site the gap analysis named as the prime
    /// suspect, whose own `validate_body_*` fixtures are all well-formed
    /// objects. (TEST-41)
    #[test]
    fn invoke_body_passes_the_shared_argument_conformance_battery() {
        use crate::common::tool_args::conformance::{assert_arg_conformance, ArgSite};
        use crate::common::tool_args::ArgShape;

        assert_arg_conformance(ArgSite {
            site: "invoke_capability.body",
            arg: "body",
            shape: ArgShape::Object,
            canonical: json!({ "name": "My project" }),
            example: INVOKE_BODY_EXAMPLE,
            absent_yields: None,
            extract: |args: Value| {
                decode_invoke_args(&args)
                    .map(|v| v.get("body").cloned().filter(|b| !b.is_null()))
                    .map_err(|e| format!("{e}"))
            },
        });
    }
}
