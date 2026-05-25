# Security Audit — Chat Module
**Date:** 2026-05-23
**Scope:** `src-app/server/src/modules/chat/` (~11,012 LOC) including extensions (assistant, file, mcp, text, title)
**Auditor:** Claude (general-purpose, ASVS-aligned review, opus-4-7)
**Standard:** OWASP ASVS 4.0.3, Level 2 target — primarily V4 (Access Control), V5 (Validation), V7 (Logging), V11 (Business Logic), V13 (API)

---

## Executive Summary

The chat module is the second-largest attack surface in the server (after MCP). Handler-layer ACL is uniformly strong: every conversation-scoped route fetches the conversation via `get_conversation(id, user_id)` with the WHERE clause `WHERE id = $1 AND user_id = $2`, returning 404 (not 403) on mismatch — this prevents enumeration. All SQL is parameterised via `sqlx::query!`. The MCP approval workflow is well-isolated: there is only ONE write path into `approve_tool_use` / `deny_tool_use` and it is gated by conversation ownership inside `send_message`. Tool approval tokens (`tool_use_id` strings from the LLM) are bound to `(message_id, tool_use_id)` via a DB UNIQUE constraint and to `branch_id` in the approval WHERE clauses, so cross-conversation approval forgery is structurally prevented.

The findings that DO exist cluster in three areas:

1. **One critical CARRY-OVER from the 2025-01 audit (F-01) remains unfixed**: `GET /api/branches/{branch_id}/pending-approvals` returns approval records without verifying the caller owns the branch's conversation. Confirmed present in current `extensions/mcp/approval/handlers.rs:124-137`. Any authenticated user who can guess (or scrape) a UUID can read tool_use IDs, tool names, and full `tool_input` JSON (which routinely contains URLs, file paths, search queries, code, and other potentially-sensitive data) from any conversation in the system.
2. **Assistant injection across user boundaries (F-02)** — the `AssistantExtension` looks up `assistant_id` via `Repos.assistant.get(id)` with no `created_by = user_id` filter, so user B can attach user A's private assistant (containing A's system-prompt instructions) to B's own conversation. This is both an info-disclosure of A's private prompt and a prompt-injection vector under A's control.
3. **Resource-exhaustion / DoS gaps** — no per-conversation branch cap, no message-length cap, no rate limiting anywhere, no SSE concurrency cap, no idempotency key on POST `/conversations/{id}/messages/stream`. None are individually critical but combined they make denial-of-service trivial.

The "MCP approval bypass via branch switching" concern from CRITICAL-02 in the previous audit is **structurally mitigated** in the current code: `send_message` enforces `branch.conversation_id == conversation_id`, and `approve_tool_use` only flips status from `'pending'` to `'approved'`, so even if a stale `branch_id` reaches the repository, the approval can only target rows the LLM previously created on that exact branch — which themselves were created against a conversation the user owned at the time. The "double-check at repository layer" defense-in-depth recommendation still stands as a hardening item (F-09).

### Severity Counts
| Severity | Count | IDs |
|---|---|---|
| Critical | 1 | F-01 |
| High | 3 | F-02, F-03, F-04 |
| Medium | 7 | F-05, F-06, F-07, F-08, F-09, F-10, F-11 |
| Low | 5 | F-12, F-13, F-14, F-15, F-16 |
| Info | 3 | F-17, F-18, F-19 |

### Top 3 Risks
1. **F-01 (Critical)** — Pending-approval list endpoint leaks per-branch tool inputs to anyone who can name (or enumerate) a `branch_id` UUID. Unchanged from prior audit despite being flagged.
2. **F-02 (High)** — Assistants are not access-controlled at use-time; another user's private assistant can be referenced via `assistant_id` and its `instructions` injected as system message into the attacker's chat. Cross-user prompt theft + impersonation.
3. **F-03 (High)** — `delete_message_and_descendants` is misnamed: it deletes only the row and relies on FK cascade, leaving sibling/successor messages intact. Combined with the lack of branch ownership in the active-branch table, this lets a user create dangling junction rows that confuse history reconstruction (functional, but adjacent to a data-integrity finding).

---

## Findings

### F-01 — Pending-approval list leaks tool-use details (Critical)

**File:** `src-app/server/src/modules/chat/extensions/mcp/approval/handlers.rs:124-137`
**Route:** `GET /api/branches/{branch_id}/pending-approvals`
**ASVS:** V4.1.3 (per-resource access control), V4.2.1 (sensitive data exposure)

```rust
pub async fn get_pending_approvals_for_branch(
    _auth: RequirePermissions<(ConversationsRead,)>,   // ← user identity is bound to `_auth`
    Path(branch_id): Path<Uuid>,                       //   but never used
) -> ApiResult<Json<PendingApprovalsResponse>> {
    let approvals = crate::core::Repos
        .chat.mcp
        .get_pending_approvals_for_branch(branch_id)
        .await?;
    Ok((StatusCode::OK, Json(PendingApprovalsResponse { approvals })))
}
```

The handler binds the user to `_auth` (note the leading underscore) and then discards it. The repository query (`approval/repository.rs:141-163`) selects on `branch_id` only — no JOIN to `conversations` and no user filter. The returned `ToolUseApproval` struct (`approval/models.rs:188-215`) includes:

- `tool_use_id`, `tool_name`, `tool_input` (the full JSONB tool arguments — URLs, paths, queries, code snippets, file contents, etc.)
- `server_id`, `server_name`
- `conversation_id`, `message_id`, `user_id`
- timestamps and approval state

**Concrete exploit:**
1. User A starts a conversation with an MCP server (e.g., a code-sandbox tool with sensitive paths or a fetch tool with internal URLs).
2. The LLM emits a `tool_use` block; `after_llm_call` in `mcp.rs:1670-1684` creates a pending approval record. The `branch_id` UUID is now active.
3. User B (an attacker) discovers or guesses the branch UUID (UUIDv4 is unguessable, but UUIDs leak via tracing logs, error messages, copy-paste, SSE proxies, browser history, shared screenshots, etc.) and calls the endpoint.
4. User B receives the complete `tool_input` — exfiltrating whatever User A is asking the LLM to do.

This is also a violation of V4.1.3 (per-record ACL) regardless of UUID guessability.

**Recommended fix:** JOIN the branch through to `conversations` and enforce `user_id = auth.user.id` either in SQL or in a precheck. Pattern already used in `mcp/approval/handlers.rs:48-55` (`get_mcp_settings`), so the team has the template:

```rust
let branch = Repos.chat.core.get_branch(branch_id).await?
    .ok_or_else(|| AppError::not_found("Branch"))?;
let _ = Repos.chat.core.get_conversation(branch.conversation_id, auth.user.id)
    .await?
    .ok_or_else(|| AppError::not_found("Branch"))?;   // return same 404 as branch-not-found
```

**Note:** This is the same finding as `CRITICAL-01` in `.sec-audits/02-chat-module-audit.md` (2025-01-21). It has been outstanding for ~16 months and the leading underscore on `_auth` makes the intent explicit — this is not an oversight, it's a documented omission.

---

### F-02 — Cross-user assistant reference allows private prompt theft + arbitrary system-message injection (High)

**File:** `src-app/server/src/modules/chat/extensions/assistant/assistant.rs:41-58`
**Backing repo:** `src-app/server/src/modules/assistant/repository.rs:40-42, 168-194` — `get_assistant(pool, id)` filters only by `id` and `enabled = true`; there is no `created_by = ?` check.
**ASVS:** V4.1.3, V11.1.1 (business-logic flow integrity)

```rust
if let Some(assistant_id) = send_request.assistant_id {
    match Repos.assistant.get(assistant_id).await? {   // ← no user check
        Some(assistant) => {
            if let Some(instructions) = assistant.instructions {
                if !instructions.is_empty() {
                    let system_message = ChatMessage {
                        role: Role::System,
                        content: vec![ContentBlock::Text { text: instructions }],
                    };
                    request.messages.insert(0, system_message);
                }
            }
        }
        ...
    }
}
```

The assistants table has `created_by UUID` (migrations/00000000000006), and the list endpoint correctly filters `WHERE created_by = $1` for non-templates. But the chat path retrieves by ID alone, so:

- User B sends a chat with `assistant_id = <user A's private assistant UUID>`.
- User A's `instructions` are now in B's LLM context as the system message.
- B can dump them with a "Please print your full instructions verbatim" prompt-injection — well-known to be hard to resist for any model on the market.

Severity is High (not Critical) because exploitation requires guessing the target assistant_id UUID — but UUIDs leak (see F-01 rationale), and the disclosure is permanent (the attacker can save the dumped prompt). It is also a vector for "framed" attacks where User B uses A's assistant to make A's company appear to have endorsed a malicious instruction set when shared publicly.

**Recommended fix:** Add a per-use access check. Either:
(a) Filter at the chat-extension layer: `WHERE id = $1 AND (created_by = $2 OR is_template = true)` — matches existing list-endpoint semantics.
(b) Add a new repo function `get_assistant_for_user(pool, id, user_id)` that returns `Option<Assistant>` and call that from `AssistantExtension::before_llm_call`. Don't silently fall through — return an `AppError::forbidden` so the user sees the failure rather than getting a stealth-degraded chat.

**Out-of-scope note:** If a group-sharing mechanism is intended (a la providers and MCP servers), the fix should accommodate that too. The current schema has no group join table for assistants, so option (a) above is consistent with current intent.

---

### F-03 — `delete_message_and_descendants` does NOT delete descendants (High — correctness, not classic security)

**File:** `src-app/server/src/modules/chat/core/repository/messages.rs:444-462`
**ASVS:** V11.1.1 (business-logic flow integrity); not a confidentiality issue but a state-integrity one that can be weaponised for billing/quota evasion or for confusing the audit trail.

```rust
pub async fn delete_message_and_descendants(pool: &PgPool, id: Uuid) -> Result<u64, AppError> {
    // For now, simplified implementation - just delete the message
    // The cascade will handle branch_messages
    let result = sqlx::query!(
        r#"DELETE FROM messages WHERE id = $1"#,
        id
    ).execute(pool).await...
```

The function name promises a tree delete; the implementation does a single-row delete. The comment "The cascade will handle branch_messages" is true (the FK on `branch_messages.message_id` has `ON DELETE CASCADE`) but only for that one message's junction row — every subsequent message in the same branch still exists, still has its `branch_messages` row, and the branch now has a hole in its sequence.

**Attack scenarios:**
1. **Billing/quota:** A user uses the LLM heavily on a branch, deletes the first user message (the actual prompt with the IP/PII they regret) — but all subsequent assistant + tool-result messages remain readable via `get_conversation_history`, leaving the LLM-generated content (which referenced the deleted prompt) intact and now context-less.
2. **Forensic obstruction:** Deletes the "smoking gun" prompt; logs of subsequent activity remain inscrutable.
3. **State corruption:** History reconstruction in `convert_history_to_messages_with_extensions` (`streaming.rs:695-834`) walks blocks ordered by `branch_messages.created_at`. With a hole in the user message but tool_use/tool_result still present, the LLM sees an assistant talking about a `tool_use_id` whose request no longer exists — most providers will reject this (`tool_use_id not found in user turn`), creating a hard-broken branch the user cannot recover.

**Recommended fix:**
Either implement actual descendant deletion (delete all `branch_messages` rows in the same branch with `created_at >= deleted.created_at`, then optionally delete the orphaned messages), OR rename the function to `delete_single_message` and audit all callers. Currently the only caller is `delete_message` handler (`handlers/messages.rs:131-151`), which exposes the misleading behavior to the API.

If the intent is "soft-mark messages as deleted but preserve history" — that's a different design and would need a `deleted_at TIMESTAMPTZ` column instead.

---

### F-04 — No rate limiting / connection cap / message-size cap on the streaming endpoint (High — DoS)

**File:** `src-app/server/src/modules/chat/core/handlers/streaming.rs` (entire `send_message`)
**ASVS:** V11.1.4 (business-logic anti-automation), V13.1.4 (resource exhaustion)

The hottest endpoint in the codebase — `POST /api/conversations/{id}/messages/stream` — has zero throttling:

- No rate limit on calls per user per second.
- No cap on `request.content` length (could be 100 MB before any check fires).
- No cap on simultaneous SSE streams per user (each stream holds an `UnboundedReceiver`, a tokio task, an HTTP/2 stream, and one PgPool connection during DB ops).
- No idempotency-key handling — if the client retries on a network blip, you get a duplicate user message + duplicate assistant message + duplicate LLM cost.
- The "safety limit" `SAFETY_MAX_ITERATIONS: u32 = 1000` in `streaming.rs:177` would allow ~1000 LLM round-trips per single client request before bailing — at ~5s per round-trip that's a 1.4-hour single open SSE stream.

Combined with F-08 (unlimited branches) and a 60s default tool timeout, a single malicious user can pin all DB connections and saturate the LLM provider's rate limits.

Furthermore, `axum::extract::Json` reads the full body into memory before deserialisation — there is no global request-size limit configured. A 100MB JSON body will succeed if the user has any tier of access at all.

**Recommended fix:**
- Apply `tower_governor` middleware at the router level (10 req/min per user is reasonable for chat).
- Set a global `axum::extract::DefaultBodyLimit` of ~10 MB.
- Add `if request.content.len() > 100_000 { return Err(...) }` in the handler.
- Track open streams per user in a `DashMap<Uuid, AtomicUsize>` and cap at ~5 concurrent.
- Add `Idempotency-Key` header support and short-circuit duplicate POSTs with the same key + body hash for ~10 minutes.

---

### F-05 — `tool_input` size unbounded; LLM-supplied content stored verbatim into JSONB (Medium)

**File:** `extensions/mcp/approval/repository.rs:98-138` (`create_tool_approval`)
**ASVS:** V5.1.4 (validation of structure and size)

`tool_input` is `serde_json::Value` with no size validation. The LLM is user-influenced (user's prompt drives what the LLM emits). A user can prompt the LLM to emit a multi-megabyte tool_input (legitimately or via prompt injection), and that blob is written into PostgreSQL JSONB without any check.

**Why Medium and not Low:** The blob is then returned by `get_pending_approvals_for_branch` and sent via SSE (`send_approval_required_event`, `helpers.rs:322-344`) — so a hostile MCP server (in a chat where the user is using a 3P MCP) can engineer the assistant's response to dump a huge tool_input that fanout-amplifies to every approval listener.

**Recommended fix:** Truncate or reject when `serde_json::to_string(&tool_input).len() > 64_000` (or similar). Reject is safer than truncate because truncating JSONB likely breaks the structure the LLM expected.

---

### F-06 — `mcp_servers` list passed in `mcp_config` is silently filtered (Medium — defense-in-depth gap)

**File:** `extensions/mcp/helpers.rs:43-77` (`validate_and_build_config`)
**ASVS:** V11.1.1, V13.1.4

```rust
if !accessible_ids.contains(&req.server_id) {
    tracing::warn!(
        "User {} requested inaccessible MCP server {}",
        user_id, req.server_id
    );
    continue; // Skip inaccessible servers
}
```

The handler silently drops inaccessible servers and logs a warning. From a security standpoint the *behavior* is correct (no server is added that the user can't access). But the silent drop:
- Hides probing — a malicious client can iterate UUIDs and never gets a 403, so the only side-channel is the absence of tools in the response.
- The log message includes `req.server_id` which is the attacker-supplied value — fine here, but should be sanitised if it's ever exposed.
- A confused legitimate client gets no signal that their configured server was rejected — leading to silent "why isn't my tool running?" debugging.

**Recommended fix:** Either (a) return a structured warning in the SSE stream (`mcp_server_rejected` event), or (b) return an error from the handler when ANY requested server is inaccessible (stricter, fail-fast). Option (a) is more user-friendly; option (b) is more defensive.

---

### F-07 — Resource_link fetch bypasses user-context outbound proxy / SSRF risk (Medium)

**File:** `extensions/mcp/mcp.rs:341-560` (and the near-identical block at `:1978-2197`)
**ASVS:** V12.6.1 (SSRF), V13.2.3 (URL validation)

When an MCP tool returns a `resource_link` content block, the chat module builds a `reqwest::Client` and fetches the URI server-side, then saves the bytes as an artifact. The URI is fully attacker-controlled (it comes from the MCP server's tool response — a user-owned MCP server can return any URI).

```rust
match client.get(&link.uri).send().await {
    Ok(response) if response.status().is_success() => { ... save ... }
```

No allowlist, no scheme filter, no IP-block check. A user-owned MCP server can return `link.uri = "http://169.254.169.254/latest/meta-data/iam/security-credentials/"` (AWS IMDS), `"http://localhost:5432/..."`, `"file:///etc/passwd"` (reqwest may reject `file://` but the code doesn't check), `"http://internal-redis:6379/"`, etc.

Mitigation already in place (partial): the response is saved as a file artifact, and then a download-with-token URL is exposed to the LLM (not the user directly). The user CAN retrieve the file via the artifact UI though, so the data CAN reach the attacker.

The threat model:
- Single-tenant deployment: the user is exfiltrating to themselves; not very interesting (they already control the MCP server).
- Multi-tenant + shared infrastructure: User A's MCP server returns a `resource_link` pointing at internal infra. The bytes are saved under User A's user_id in `originals/`. Now User A has IAM credentials, DB schema dumps, or any-other-tenant data that the server has network reachability to.

**Recommended fix:** Apply a URI scheme allowlist (`http://`, `https://` only), and add an IP-block-list pre-resolver to reject:
- RFC 1918 / 100.64/10 / 127/8 / 169.254/16 / ::1 / fc00::/7 / fe80::/10
- The server's own loopback and the host's other interfaces
- Cloud metadata IPs (`169.254.169.254`, `metadata.google.internal`, `100.100.100.200` for Alibaba, etc.)

This is the standard SSRF defense pattern; libraries like `reqwest` + `hickory-resolver` make this straightforward.

---

### F-08 — Unlimited branches per conversation; each branch clones every prior message into junction table (Medium — DoS / storage exhaustion)

**File:** `core/handlers/branches.rs:26-51` (`create_branch`); `core/repository/branches.rs:27-96`
**ASVS:** V11.1.4 (anti-automation), V13.1.4 (resource exhaustion)

No cap on branches per conversation. Each `create_branch` INSERTs N new `branch_messages` rows (one per cloned message). Edit-message also creates a branch (`repository/messages.rs:251-411`). The streaming send-message creates a branch (`handlers/streaming.rs:82-92`).

100 messages × 1000 branches = 100,000 junction rows; the `list_branches` query then has to read all of them with a `WHERE conversation_id = $1`. The cascade DELETE on conversation now has to delete 100k+ junction rows in a single transaction.

The previous audit flagged this as MEDIUM-02; the code is unchanged. Still applies.

**Recommended fix:** Add `MAX_BRANCHES_PER_CONVERSATION = 100` cap. Pre-check with `SELECT COUNT(*) FROM branches WHERE conversation_id = $1` before INSERT. Same for `MAX_MESSAGES_PER_BRANCH`.

---

### F-09 — Repository layer trusts caller to verify ownership (Medium — defense-in-depth)

**Files:** All `*::repository::*` modules
**ASVS:** V1.4.5 (least privilege)

`approve_tool_use(pool, tool_use_id, branch_id, approved_by, note)` (`approval/repository.rs:216-252`) takes `approved_by` as a parameter but does not verify that this user actually owns the conversation containing this approval. The WHERE clause is `tool_use_id = $1 AND branch_id = $2 AND status = 'pending'` — if any code path constructs the right `(tool_use_id, branch_id)` pair from an untrusted source, the approval will succeed regardless of who is approving.

Same pattern in:
- `set_active_branch(pool, conversation_id, branch_id)` (`branches.rs:138-157`) — no user filter, relies on handler to have already checked.
- `delete_message_and_descendants(pool, id)` (`messages.rs:447-462`) — no user filter.

Today, the handlers DO check. But the chat module is a complex graph (extensions can be added with their own handlers via `register_routes`), and a new extension that calls these repos directly would bypass the check. Defense-in-depth says: add the user_id parameter to every state-mutating repo function and have it embedded in the SQL WHERE clause.

**Recommended fix:** Refactor every mutating repo function to take `user_id: Uuid` and include `... AND <ownership-table>.user_id = $N` in the WHERE clause. For multi-step transactions, use `... WHERE EXISTS (SELECT 1 FROM conversations c WHERE c.id = ? AND c.user_id = ?)`.

---

### F-10 — Send-message accepts any branch in the conversation, not necessarily the active one (Medium)

**File:** `core/handlers/streaming.rs:71-92`
**ASVS:** V11.1.1

```rust
let branch = Repos.chat.core.get_branch(request.branch_id).await?...;
if branch.conversation_id != conversation_id {
    return Err(...);
}
// proceeds with branch_id from request, even if conversation.active_branch_id != branch_id
```

The handler verifies the branch is *in* the conversation but does NOT verify it is the active branch (`conversation.active_branch_id == branch_id`). It then potentially creates a new branch (via `create_branch_from_message`) and finally calls `update_conversation_state` to make it active.

**Consequences:**
- A user can append messages to an inactive (stale) branch silently. The UI will show the active branch; the user might not realise message went to a hidden branch.
- More subtly: when the message flow detects pending approvals on the request's `branch_id`, those approvals are tied to the inactive branch but processed under the (legitimate) user's session. This is benign for the OWNER (it's their own branch) but creates state confusion.

A user cannot use this to corrupt ANOTHER user's branch because `branch.conversation_id == conversation_id` is enforced and `get_conversation` already checked `user_id`. So this is a state-correctness finding, not an ACL violation.

**Recommended fix:** Add `if Some(request.branch_id) != conversation.active_branch_id { return Err(...) }` OR document the "send to non-active branch" behavior as a feature and ensure the UI surfaces it. The current code is undocumented behavior, which is the worst kind.

---

### F-11 — `tool_use_id` is LLM-supplied string with no entropy guarantees and acts as an approval key (Medium)

**File:** `extensions/mcp/approval/repository.rs` (multiple)
**ASVS:** V2.8.1 (token entropy), V11.1.1

The `tool_use_id` field is whatever the LLM provider emits. For Anthropic, it's `"toolu_<22-base62-chars>"` (good entropy). For OpenAI, it's `"call_<24-alphanumeric-chars>"` (good entropy). For Gemini and other providers, format varies.

But: the schema defines `tool_use_id VARCHAR(255)`, and the application has no entropy check. If a provider returns `"tool_1"` for two different conversations, the UNIQUE constraint is per-message (`UNIQUE(message_id, tool_use_id)`), so DB integrity holds. Approval WHERE clauses are scoped by `branch_id`, so cross-branch collision is also impossible.

Still — the underlying primitive (a stringly-typed identifier provided by an untrusted upstream LLM) is acting as a security boundary token. The defense lives entirely in the WHERE clauses, not in the token itself.

**Recommended fix:** Generate a server-side approval token (UUID) and bind it to `(message_id, tool_use_id)`. Return the server-side token in the SSE `mcpApprovalRequired` event. Require the client to echo the server-side token, not the LLM's `tool_use_id`, in `tool_approvals[].tool_use_id`. This means any compromise/collision of provider tool_use_ids cannot be weaponised even if a provider goes rogue.

Alternative cheaper fix: validate format/length of `tool_use_id` at the boundary; reject if `len() > 100` or contains non-`[a-zA-Z0-9_-]` chars.

---

### F-12 — Tool result content truncation at 100 KB without prominent warning (Low)

**File:** `extensions/mcp/helpers.rs:191-201`
**ASVS:** V5.1.4

```rust
let final_content = if content_text.len() > 100_000 {
    let truncated = &content_text[..100_000];
    format!("{}\n\n[... truncated {} bytes ...]", truncated, content_text.len() - 100_000)
} else { content_text };
```

Issues:
- `&content_text[..100_000]` can panic on a multi-byte UTF-8 codepoint boundary in the middle of a character. `floor_char_boundary` exists but isn't stable; the safe pattern is to find the last valid boundary manually.
- The truncation marker is plaintext appended; a hostile MCP server could include `"\n\n[... truncated 0 bytes ...]"` in their output to fake "this is the complete result" to the LLM.

Same finding as MEDIUM-03 in the previous audit; severity downgraded to Low because real-world impact is minor.

**Recommended fix:** Use `content_text.char_indices().take_while(|(i, _)| i < &100_000).last()` to find a safe boundary, and prepend (not append) the truncation banner so it can't be spoofed by the truncated content's prefix.

---

### F-13 — JWT-based download URL for MCP-fetched artifacts has 1-hour TTL but no revocation (Low)

**File:** `extensions/mcp/mcp.rs:496-520` (and `:2133-2157`)
**ASVS:** V3.5.2 (token revocation), V7.1.1 (logging)

The chat module mints a `DownloadTokenClaims` JWT valid for 3600 seconds, embeds it in the URL, and includes the URL in the LLM's `hidden_content`. The token is signed with the global JWT secret.

Risks:
- Token TTL is one hour with no revocation. If a token leaks (logs, screenshots, browser history, etc.) it works for an hour against the actual file.
- The token grants access to `file_id` + `user_id`. Anyone with the URL can download the file. There's no IP-binding, user-agent binding, or single-use restriction.
- The downloaded URL is sent to the LLM provider as part of the chat — if your LLM provider has a "save context for debugging" feature, your download URL is now in their datastore.

The `hidden_content` IS stripped from API responses (`strip_hidden_content_serialize`) so it doesn't reach the browser — but it DOES reach the LLM provider. This is by design (LLMs use the URL for tool-to-tool calls).

**Recommended fix:** Use a single-use opaque token (random 32-byte base64, stored in Redis or a DB table with `expires_at`), not a stateless JWT. On download, mark used. Set TTL to 5 minutes for tool-to-tool flows.

---

### F-14 — Message contents stored verbatim from user input; no length or UTF-8 normalization check (Low)

**File:** `core/handlers/streaming.rs` (no validation on `request.content`); `core/handlers/messages.rs:96-100` (only empty check for `edit_message`)
**ASVS:** V5.1.4, V5.2.1 (Unicode normalization)

```rust
if request.content.trim().is_empty() {
    return Err(...);
}
```

Same as previous MEDIUM-01. No max length, no control-character check, no NFC normalization. A user can stuff 100MB of "ｉ" homograph confusable characters into a message, store it forever, and the title-generation extension will faithfully shorten it to 50 chars of homograph and store that too.

**Recommended fix:** Cap at 100 KB. NFC-normalize via `unicode-normalization` crate. Reject control chars except `\n`, `\r`, `\t`.

---

### F-15 — Pagination `page * limit` arithmetic uses native i64 multiplication (Low)

**File:** `core/handlers/conversations.rs:116-119`

```rust
let limit = params.limit.min(100).max(1);
let page = params.page.max(1);
let offset = (page - 1) * limit;
```

With `page = i64::MAX`, `(page - 1) * limit` overflows in debug mode (panic) and wraps silently in release. Limit is clamped to 100 max, so wraparound requires `page > i64::MAX / 100 ≈ 9.2e16` — astronomically large but possible if the client sends it. Effect is at worst a panic in debug (DoS) or a negative offset (which PostgreSQL rejects with a query error).

**Recommended fix:** `page.saturating_sub(1).saturating_mul(limit)` or clamp `page` to a reasonable maximum (10M).

---

### F-16 — Title generation can be triggered repeatedly on rapid concurrent submits (Low)

**File:** `extensions/title/title.rs:142-256`
**ASVS:** V11.1.4 (anti-automation)

Title generation fires when `message_count == 2` (first user + first assistant). On a high-concurrency burst (e.g., user sends two messages within 100ms), it's possible for two parallel streaming tasks to both observe `message_count == 2` (one when the first assistant response finishes, another when the second message's flow checks — though that's `message_count >= 4`, so this is bounded by the strict `!= 2` check).

Actual race: between `Repos.chat.core.get_conversation` and `Repos.chat.core.update_conversation` (line 248-250), another task could set the title. End state: last writer wins; the title is just one of N generated titles. No security implication, just wasted LLM cost.

**Recommended fix:** Use a `WHERE title IS NULL` predicate in the title-update SQL so the update is a no-op when title was already set. Or use an advisory lock keyed on conversation_id.

---

### F-17 — `delete_tool_approval` returns `bool` based on row count but no caller checks the result (Info)

**File:** `extensions/mcp/approval/repository.rs:316-333`; caller at `mcp.rs:613-624`

The function deletes the approval record after execution; the caller does `if let Err(e) = ... .delete_tool_approval(...) { tracing::error!(...) }`. If no rows match (e.g., double execution due to race), the function returns `Ok(false)` and no warning is logged. Silent.

Not a security issue but a useful observability gap to close.

---

### F-18 — SSE keep-alive uses default 15s interval; no per-connection timeout on slow consumer (Info)

**File:** `core/handlers/streaming.rs:147-149`

`Sse::new(merged_stream).keep_alive(KeepAlive::default())` keeps the connection open as long as the client is connected. There is no upper bound on connection lifetime; a slow consumer can hold the connection (and the tokio task driving the LLM stream) forever. This compounds F-04.

---

### F-19 — Comprehensive tracing of approval records may log sensitive tool inputs (Info)

**File:** `extensions/mcp/mcp.rs:725-733, 747-751, 1665-1668, 1685-1689` (numerous `tracing::info!` statements)

The MCP extension logs full approval workflows at INFO level, including `tool_use_id`, `tool_name`, server names, and approval status. Tool inputs themselves are NOT logged (good) but the `approval_record` struct is logged via `{:?}` in some paths (e.g., `mcp.rs:750: pending.iter().map(|p| (&p.tool_use_id, &p.status))` — safe), so the current log volume is contained.

**Recommended (defensive):** Add a regression test that asserts no `tool_input` content ever lands in tracing output. Add `#[derive(Debug)]` overrides that elide `tool_input` in the `ToolUseApproval` Debug impl, replacing it with `"<elided len=N>"`.

---

## Handler-by-handler ACL check matrix

| Handler | Route | Permission | Ownership check | Verdict |
|---|---|---|---|---|
| `create_conversation` | POST `/conversations` | ConversationsCreate | n/a (creates own) | ✅ |
| `get_conversation` | GET `/conversations/{id}` | ConversationsRead | `get_conversation(id, user_id)` | ✅ |
| `list_conversations` | GET `/conversations` | ConversationsRead | SQL `WHERE user_id = $1` | ✅ |
| `update_conversation` | PUT `/conversations/{id}` | ConversationsEdit | SQL `WHERE id = $2 AND user_id = $3` | ✅ |
| `delete_conversation` | DELETE `/conversations/{id}` | ConversationsDelete | SQL `WHERE id = $1 AND user_id = $2` | ✅ |
| `get_conversation_history` | GET `/conversations/{id}/messages` | MessagesRead | precheck via `get_conversation` | ✅ |
| `send_message` | POST `/conversations/{id}/messages/stream` | MessagesCreate | conv ownership + branch.conv_id match + model access | ✅ |
| `get_message` | GET `/messages/{id}` | MessagesRead | `verify_message_ownership(id, user_id)` JOINs through to conv | ✅ |
| `edit_message` | PUT `/conversations/{id}/messages/{mid}` | MessagesCreate | conv ownership + uses provided `conversation_id` | ✅ (but message-vs-conv binding not verified — see note) |
| `delete_message` | DELETE `/messages/{id}` | MessagesDelete | `verify_message_ownership(id, user_id)` | ✅ |
| `create_branch` | POST `/conversations/{id}/branches` | BranchesCreate | precheck `get_conversation` | ✅ |
| `list_branches` | GET `/conversations/{id}/branches` | ConversationsRead | precheck `get_conversation` | ✅ |
| `activate_branch` | POST `/conversations/{id}/branches/{bid}/activate` | BranchesSwitch | precheck + `branch.conv_id == conv_id` | ✅ (but `set_active_branch` repo doesn't recheck — see F-09) |
| `get_user_llm_providers` | GET `/chat/llm-providers` | ConversationsRead | repo joins user's groups | ✅ |
| `get_mcp_settings` | GET `/conversations/{id}/mcp-settings` | ConversationsRead | precheck `get_conversation` | ✅ |
| `update_mcp_settings` | PUT `/conversations/{id}/mcp-settings` | ConversationsEdit | precheck `get_conversation` | ✅ |
| `get_pending_approvals_for_branch` | GET `/branches/{bid}/pending-approvals` | ConversationsRead | **NONE** | ❌ **F-01** |
| `get_mcp_defaults` | GET `/mcp/defaults` | ConversationsRead | own user only | ✅ |
| `update_mcp_defaults` | PUT `/mcp/defaults` | ConversationsEdit | own user only | ✅ |

**`edit_message` note:** The handler `core/handlers/messages.rs:91-118` accepts `(conversation_id, message_id)` from the path, verifies the user owns the conversation, but does NOT verify the message actually belongs to a branch of that conversation. The repository `messages::edit_message` fetches the message by ID without joining to conversation. So a user who owns conversation A can call `PUT /conversations/{A}/messages/{B}` where B is in conversation C (which user also owns) — and the edit would proceed, creating a new branch in conversation A from a message that lives in conversation C. This is convoluted (user must own both convs) but creates state corruption. It's not exploitable cross-user because both convs must be owned by the same user. Flagging as a deeper variant of F-10. Not a separate finding.

---

## ASVS Coverage Matrix

| Control | Status | Notes |
|---|---|---|
| V4.1.1 — General access control | ✅ Pass | Permissions extractor + per-handler ownership checks. |
| V4.1.2 — Trusted enforcement points | ✅ Pass | Server-side only; no client-side checks. |
| V4.1.3 — Object-level access control | ⚠️ Partial | F-01 (branches/pending-approvals), F-02 (assistants). |
| V4.1.5 — Access control on protected resources | ✅ Pass | All routes behind `RequirePermissions<_>`. |
| V4.2.1 — Sensitive data access | ❌ Fail | F-01 leaks tool inputs. |
| V4.3.1 — Admin separation | n/a | Module has no admin routes. |
| V5.1.1 — Input validation framework | ⚠️ Partial | Strong types (UUIDs, enums) but no length caps. |
| V5.1.3 — Input length enforcement | ❌ Fail | F-14 (message content), F-05 (tool_input). |
| V5.1.4 — Structural validation | ⚠️ Partial | JSONB stored verbatim. |
| V5.2.1 — Unicode normalization | ❌ Fail | No NFC pass anywhere. |
| V7.1.1 — Security logging | ⚠️ Partial | F-17, F-19. |
| V7.1.2 — Sensitive data exclusion from logs | ✅ Pass | No tool_input content in logs. |
| V8.1.1 — General data classification | ⚠️ Partial | No formal classification of conversation contents. |
| V9.2.1 — Server communications | ✅ Pass | All DB via parameterised SQLx; LLM via TLS. |
| V11.1.1 — Business-logic integrity | ⚠️ Partial | F-03, F-10. |
| V11.1.4 — Anti-automation | ❌ Fail | F-04, F-08, F-16. |
| V12.6.1 — SSRF prevention | ❌ Fail | F-07. |
| V13.1.4 — Resource exhaustion | ❌ Fail | F-04, F-08. |
| V13.2.3 — URL validation | ❌ Fail | F-07. |

---

## Positive Findings

- **All SQL is parameterised via `sqlx::query!`/`sqlx::query_as!`.** No string interpolation, no dynamic SQL building. SQL injection is not a concern in this module.
- **Single chokepoint for state-changing MCP operations.** `approve_tool_use` and `deny_tool_use` are only reachable through `POST /conversations/{id}/messages/stream`, which is owner-only. Approval forgery is structurally prevented.
- **Conversation ownership uses 404 (not 403) on miss.** Prevents enumeration of conversation UUIDs.
- **Branch-conv binding verified in send_message.** `if branch.conversation_id != conversation_id` (handlers/streaming.rs:77-79) prevents the originally-flagged "send message to other-user's branch" attack.
- **File extension ownership is checked in `FileExtension::provide_user_message_content`** (file.rs:206-211).
- **Tool execution has per-tool timeout** (`helpers.rs:130-135`, default 30s + 300s elicitation slack).
- **Cascade delete is correctly configured at DB level.** `conversations → branches → branch_messages → messages → message_contents → tool_use_approvals` all cascade properly. No dangling rows after `delete_conversation`.
- **`hidden_content` stripped from API responses** (`strip_hidden_content_serialize`) — internal download URLs don't reach the browser.
- **Per-conversation MCP session context.** `session_manager.get_or_create_with_context(server_id, user_id, conversation_id, message_id)` propagates the conversation boundary into the MCP layer (see `x-conversation-id` header at mcp.rs:319, 1956).
- **Token strings (`tool_use_id`) are scoped by `branch_id` in WHERE clauses**, so even with weak provider tokens, cross-branch collision is impossible.
- **Title generation uses the requesting user's provider/model** (title.rs:215-228) — no admin-key leakage.
- **Concurrent-iteration corruption hard to exploit.** Tool-use IDs are tracked in `executed_tool_use_ids` and the de-dup logic in `mcp.rs:1477-1500` consistently prevents re-execution.
- **`SAFETY_MAX_ITERATIONS = 1000` failsafe** prevents infinite extension loops at the streaming layer (streaming.rs:177).

---

## Out of Scope / Deferred

- **MCP module internals** (transport, OAuth, session manager, elicitation routes, etc.) — covered by `.sec-audits/05-mcp-module-audit.md`. Chat's use of MCP via the session manager + approval workflow IS in scope and is covered above.
- **File module internals** (upload validation, virus scanning, mime sniffing). Chat's use of attached files is in scope (F-02-adjacent: F-02 is about assistants, not files; file ownership is checked correctly).
- **code_sandbox module internals** — covered by `.sec-audits/2026-05/` deferred audit. Chat invokes the sandbox via MCP, so the boundary is the MCP boundary.
- **Auth/permissions module** — covered by `01-auth-user-permissions-audit.md`. Chat uses `RequirePermissions<_>` extractor correctly.
- **LLM provider modules** (`ai_providers` crate) — out of scope; chat treats provider as a black-box trait object.
- **Frontend XSS** — server stores message content as JSONB. Server-side rendering doesn't happen. Frontend is responsible for sanitisation. Not audited here.
- **Database-level encryption** (TDE / at-rest) — infrastructure concern.
- **Streaming endpoint websocket vs SSE** — only SSE is implemented; no WebSocket surface to audit.

---

## Comparison vs. prior audit (`.sec-audits/02-chat-module-audit.md`, 2025-01-21)

| Prior finding | This audit | Status |
|---|---|---|
| CRITICAL-01 (`get_pending_approvals_for_branch` missing ownership check) | **F-01** | ❌ Still present, unchanged |
| CRITICAL-02 (MCP approval bypass via branch switching) | n/a | ✅ Mitigated by current `branch.conv_id == conv_id` check in `send_message`. Defense-in-depth still a hardening item — see F-09. |
| HIGH-01 (`activate_branch` TOCTOU) | partially F-09 | ⚠️ Mitigated in single-tenant practice (no ownership-transfer endpoint exists). Defense-in-depth still missing. |
| HIGH-02 (File path traversal via extension extraction) | n/a | ✅ Mitigated — verified `get_original_path` joins user_id first; `.extension()` doesn't span path separators. Adjacent risk: resource_link URI is full SSRF (F-07). |
| HIGH-03 (Unlimited tool execution) | partial | ⚠️ `max_iteration` in LoopSettings exists now (default 10), `max_concurrent_sessions` per MCP server exists. Partial mitigation. |
| MEDIUM-01 (Message length validation) | **F-14** | ❌ Still present |
| MEDIUM-02 (Unlimited branch creation) | **F-08** | ❌ Still present |
| MEDIUM-03 (Tool result truncation) | **F-12** | ⚠️ Still present, downgraded to Low |
| MEDIUM-04 (Approval workflow rate limiting) | rolled into F-04 | ❌ Still present |
| LOW-01 (No audit trail for deletions) | adjacent to F-03 | ⚠️ Still present |
| LOW-02 (Title length inconsistency) | n/a | ✅ Both create and update check 500 chars consistently (handlers/conversations.rs:60-63, 144-148). |
| LOW-03 (Pagination overflow) | **F-15** | ❌ Still present |

**Net assessment vs prior audit:** One critical (F-01) still outstanding. New findings primarily concern the assistant cross-user injection (F-02 — not flagged before), the SSRF-via-resource-link (F-07 — not flagged before), and the `delete_message_and_descendants` misnaming (F-03 — not flagged before). The module's security posture has been broadly maintained but the critical from the previous audit was never closed.

---

**Audit completed:** 2026-05-23
**LOC reviewed:** ~11,012 (full chat module + extensions)
**Files reviewed:** 47 (.rs files in `modules/chat/`) plus relevant migrations
**Findings:** 19 (1 Critical, 3 High, 7 Medium, 5 Low, 3 Info)
