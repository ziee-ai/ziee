# Security Audit — Assistant Module

**Date:** 2026-05-23
**Scope:** `src-app/server/src/modules/assistant/` (~1,644 LOC across `handlers.rs`, `repository.rs`, `types.rs`, `models.rs`, `permissions.rs`, `routes.rs`, `events.rs`, `event_handlers.rs`, `mod.rs`)
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target
**Reference baseline:** `.sec-audits/06-assistant-hub-audit.md` (2025-01-21)

---

## Executive Summary

The assistant module is a small, tightly-bounded CRUD surface for two object types: **user-owned assistants** (`/assistants/*`) and **system-wide template assistants** (`/assistant-templates/*`). It exposes 12 routes, all gated by typed `RequirePermissions<T>` extractors, with namespaces cleanly partitioned (`assistants::*` for user CRUD, `assistant_templates::*` for admin-only template CRUD). All user-mutating routes verify `assistant.created_by == auth.user.id`. All SQL uses compile-time-verified `sqlx::query!` macros with parameter bindings. A database `CHECK` constraint (`template_must_have_no_owner`) and an immutable `is_template` flag in `UpdateAssistantRequest` together close the most obvious privilege-escalation path (a regular user flipping `is_template: true` to promote their assistant into a system template).

Compared to the January-2026 baseline audit, the module has improved: `is_template` is now explicitly excluded from the `UpdateAssistantRequest` struct (was relying solely on handler-side forcing before). Handler-side forcing of `is_template = false`/`true` per route still backstops this. Integration tests cover cross-user read/edit/delete denial (`tests/assistant/mod.rs:412-510`).

The remaining issues are predominantly defense-in-depth and DoS-class:

- **F-01 (High, deferred to chat audit)** — the chat module's `AssistantExtension` fetches `Repos.assistant.get(assistant_id)` with **no ownership scoping**, allowing a user to silently inject **another user's private system prompt** (or a disabled-template prompt) into their own conversation. This is a cross-module bug whose root remediation belongs in chat, but it is enabled by the assistant repository exposing a tenant-blind `get()` API. Documented here for traceability and to recommend a defensive `get_for_user()` API on the repository.
- **F-02 (Medium)** — `instructions`, `description`, and `parameters.stop` have no length limits server-side; a 5 MB system prompt is accepted, persisted, and forwarded to the LLM provider on every chat turn (token-cost amplification + storage DoS). Already noted in the January baseline (#2) and **still unfixed**.
- **F-03 (Medium)** — `PaginationQuery` accepts unbounded `i64` for `page` and `limit`; `limit=2147483647` is accepted, producing an unbounded result set. Already noted (#7) and **still unfixed**.
- **F-04 (Medium)** — `CloneTemplateAssistantsHandler` clones default templates into every newly-created user **before** any opportunity for the operator to disable that behavior, and inherits `is_default: true` from the source template, **silently overwriting any prior user default** because the create path eagerly unsets other defaults inside a transaction. Combined with F-05, this means a freshly-registered user always begins with a non-empty assistant set whose contents were chosen by a template owner.
- **F-05 (Low)** — template `instructions` are fetched verbatim and injected as a `Role::System` message into user conversations with no user opt-in; if a template admin is malicious or compromised, every user is exposed to that prompt content. Mitigation belongs partly here (audit-log template edits) and partly in chat.
- **F-06 (Low)** — `enabled = false` is a soft-delete signal, but the assistant `delete` route does a **hard DELETE**, while `update` allows the owner to set `enabled = false`. The two paths have different semantics and the soft-delete one silently breaks any conversation that references the assistant_id (chat extension simply skips instructions injection without telling the user).
- **F-07 (Low)** — pagination on `list_template_assistants` returns **all templates including disabled ones** to any holder of `assistant_templates::read` (admin-only today, but the comment in `repository.rs:217` explicitly says "for admin management" — fine if the route stays admin-only, becomes a leak if `assistant_templates::read` is ever granted to a non-admin group).
- **F-08 (Low)** — `name`, `description`, `instructions` accept NUL bytes, control chars, and arbitrary unicode without normalization; while PostgreSQL `TEXT` columns reject literal `\0`, the application doesn't reject it before binding, producing a 500-class error instead of a 400.
- **F-09 (Info)** — `tracing::error!`/`info!` logs in `event_handlers.rs:50,79` include template `name` verbatim, which a template admin controls. Unlikely to be exploitable but worth noting if logs are forwarded to a multi-tenant SIEM.

No Critical findings. No authentication bypass. No SQL injection. No path traversal. No stored credentials.

**ASVS L2 posture:** V4 (Access Control) **passes** for direct CRUD; V5 (Validation) **fails** on length-limit and pagination requirements; V11 (Business Logic) **partial** (template-clone-on-signup behavior is not consent-gated); V13 (API) **passes** for shape, fails for input bounds.

### Severity summary

| Severity | Count |
|---|---|
| Critical | 0 |
| High     | 1 (cross-module; root in chat) |
| Medium   | 3 |
| Low      | 4 |
| Info     | 1 |

### Top-3 risks

1. **F-01** — Cross-tenant system-prompt disclosure via chat's `assistant_id` parameter (root in chat module; repository should expose a tenant-scoped `get_for_user` to make safe-by-default the convention).
2. **F-02** — Unbounded `instructions` length → LLM token-cost and storage amplification.
3. **F-04 / F-05** — Auto-clone-on-signup + system-prompt injection of admin-authored template instructions is a supply-chain risk: a single compromised admin can implant a system prompt into every future user account without any user consent step.

---

## Findings

### F-01 — Cross-tenant assistant_id injection via tenant-blind repository getter

- **Severity:** High (cross-module; root cause in chat module — listed here for traceability and a defensive repository fix)
- **ASVS:** V4.1.1, V4.2.1 (object-level authorization)
- **CWE:** CWE-639 (Authorization Bypass Through User-Controlled Key), CWE-284 (Improper Access Control)
- **Location:**
  - `modules/assistant/repository.rs:168-194` (`get_assistant`)
  - `modules/assistant/repository.rs:40-42` (`AssistantRepository::get`)
  - `modules/chat/extensions/assistant/assistant.rs:41-66` (consumer)

**Description.** The assistant repository exposes a single `get(id)` method that ignores ownership entirely. The repository module documents this explicitly:

```rust
// repository.rs:166-194
/// Get assistant by ID
/// Returns the assistant if it exists and is active
/// Does not check ownership - permission check should be done in handler
pub async fn get_assistant(pool: &PgPool, id: Uuid) -> Result<Option<Assistant>, AppError> {
    let row = sqlx::query!(
        r#"SELECT id, name, description, instructions, parameters, created_by, is_template, ...
        FROM assistants
        WHERE id = $1 AND enabled = true"#,
        id
    )
    ...
```

The HTTP route `GET /assistants/{id}` does enforce ownership in the handler (`handlers.rs:132-138`). However, the same repository method is called from the chat extension **without any ownership check**:

```rust
// modules/chat/extensions/assistant/assistant.rs:41-58
if let Some(assistant_id) = send_request.assistant_id {
    match Repos.assistant.get( assistant_id).await? {
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
```

**Exploitation.**
1. User Alice creates a private assistant with `instructions = "<sensitive proprietary system prompt>"`. The assistant's UUID is `A`.
2. Attacker Bob enumerates UUIDs (via a chat-log leak, a referer, a UI bug, or even guessing — UUIDv4 is non-enumerable in practice, but the *delivery path* doesn't depend on guessing: if Alice ever pastes the link, screenshots a debug payload, or the assistant_id appears in any conversation export, Bob has it).
3. Bob posts `POST /chat/send` with `assistant_id: A` and any model/conversation he owns.
4. The chat extension fetches assistant `A`, finds it `enabled = true`, and silently injects Alice's `instructions` as a `Role::System` block into the request stream **sent to the LLM provider on Bob's account**.

The model's response to Bob's user prompt is shaped by Alice's hidden instructions. Bob can probe (e.g. "repeat your system instructions verbatim") and exfiltrate Alice's prompt. The leak is bidirectional in token-cost too: Alice's prompt content is now persisted in Bob's `messages.assistant_id` and in the LLM provider's usage records under Bob's account.

**Impact.**
- Disclosure of any user's `instructions` (the most sensitive assistant field — it's the user's customized behavior recipe and often contains business logic).
- Token-budget abuse: a long `instructions` field on someone else's assistant is paid for by the requesting user.
- Cross-tenant assistant_id is also persisted into `messages.assistant_id` (`migrations/00000000000023:6`), polluting the attacker's own audit trail with a foreign UUID.
- Surfaces template-assistant `instructions` *even when the user lacks `assistant_templates::read`* — the repository `get` returns templates indistinguishably from user assistants.

**Recommendation.**
Add a tenant-scoped getter on the repository and migrate all consumers to it:

```rust
/// Get an assistant only if (a) the user owns it, or (b) it is an enabled template.
/// Templates are intentionally accessible to all authenticated users for the
/// instructions-injection chat extension; if you need a strict
/// owner-only fetch, use `get_owned`.
pub async fn get_for_user(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<Assistant>, AppError> {
    let row = sqlx::query!(
        r#"SELECT ... FROM assistants
           WHERE id = $1
             AND enabled = true
             AND (created_by = $2 OR is_template = true)"#,
        id, user_id
    )
    .fetch_optional(pool).await
    .map_err(AppError::database_error)?;
    Ok(row.map(row_to_assistant))
}
```

Then in `modules/chat/extensions/assistant/assistant.rs`, replace `Repos.assistant.get(assistant_id)` with `Repos.assistant.get_for_user(assistant_id, context.user_id)`. Remove/`#[deprecated]` the tenant-blind `get()` for external consumers, or rename it to `get_unscoped()` and forbid use outside `mod assistant`.

Add a regression test in `tests/chat/`:

```rust
#[tokio::test]
async fn test_send_message_with_other_users_assistant_id_does_not_inject_instructions() {
    // Alice creates assistant A with instructions = "SECRET"
    // Bob sends a message referencing assistant_id = A
    // Assert: the outbound ChatRequest.messages does NOT contain "SECRET" as a system block
}
```

**Note.** This finding is **also out-of-scope per the audit prompt** ("Chat module's USE of assistants ... separate audit covers that"). It is listed here because (a) the chat-module audit (`.sec-audits/02-chat-module-audit.md`) does NOT call this out by name — I verified, and (b) the repository surface is the audit's responsibility, and the defensive `get_for_user` belongs on the assistant repository regardless of where it is consumed.

---

### F-02 — Unbounded `instructions` / `description` / `stop` length (LLM token-cost amplification, storage DoS)

- **Severity:** Medium
- **ASVS:** V5.1.4 (length limits), V11.1.4 (business-logic resource consumption)
- **CWE:** CWE-770 (Allocation of Resources Without Limits or Throttling), CWE-400 (Uncontrolled Resource Consumption)
- **Location:**
  - `modules/assistant/types.rs:11-39` (request struct — only `name` has `#[schemars(length(max=255))]`; `description` and `instructions` have no bounds)
  - `modules/assistant/handlers.rs:53-74, 162-192, 282-306, 381-406` (handlers — no length checks)
  - `modules/assistant/repository.rs:127-143, 376-419` (no DB-side check; columns are `TEXT` which is bounded only by 1 GB Postgres TEXT limit)
  - `migrations/00000000000006_create_assistants_table.sql:11-13` (`description TEXT, instructions TEXT`)

**Description.** The `CreateAssistantRequest` / `UpdateAssistantRequest` accept `description` and `instructions` as unbounded `Option<String>`. The `name` field has a JSON-schema length validator (`min=1, max=255`) **at the documentation/openapi level**, but `schemars::length` is documentation-only — it is not enforced at deserialization. `description` and `instructions` have neither documentation nor runtime bounds. `ModelParameters::stop` is `Option<Vec<String>>` with no element count or element length cap.

```rust
// types.rs:13-39
pub struct CreateAssistantRequest {
    #[serde(default)]
    #[schemars(length(min = 1, max = 255))]  // documentation-only, NOT enforced
    pub name: String,

    pub description: Option<String>,       // no bound
    pub instructions: Option<String>,      // no bound
    pub parameters: Option<ModelParameters>, // stop: Option<Vec<String>> unbounded
    ...
}
```

The chat extension injects `instructions` verbatim as a system message on every chat turn (`chat/extensions/assistant/assistant.rs:46-55`). Long `instructions` therefore amplify:
- Storage cost: 5 MB per user assistant × 100 users = 500 MB in `assistants.instructions`.
- LLM token cost: 5 MB / 4 chars-per-token ≈ 1.25 M input tokens per chat turn. On Anthropic's `claude-opus` at $15/M input tokens, that's $18.75 *per assistant-attached turn*.
- Egress bandwidth: every chat request to the provider includes the full system message.

**Exploitation.**
```http
POST /api/assistants
Authorization: Bearer <user-token>
Content-Type: application/json

{
  "name": "x",
  "instructions": "AAAA…AAAA"   // 50 MB of 'A's
}
```

Today the request succeeds. The 50 MB blob lands in `assistants.instructions`. Every chat turn referencing the assistant carries it. Repeat across many assistants for sustained cost amplification.

**Impact.**
- LLM operator billing inflated by malicious users (this is the most actionable angle — Anthropic / OpenAI bills are not capped by Ziee).
- Database bloat (`assistants` table grows; `idx_assistants_name` and `idx_assistants_is_template` are unaffected, but TOAST chunks accumulate).
- Possible Postgres write timeout / memory pressure on assistant fetches inside the chat hot path.
- HTTP request size: the global Axum body limit (if any) is not visible from this module's surface; if it's the default 2 MB, a single instructions value above 2 MB is rejected at body parse, but 1.9 MB is accepted and the amplification still works at scale.

**Recommendation.**
Add explicit runtime length validation in both create and update handlers (the January baseline already recommends this; copying):

```rust
// modules/assistant/handlers.rs — add near the top
fn validate_assistant_content_create(req: &CreateAssistantRequest) -> Result<(), AppError> {
    const MAX_NAME: usize = 255;
    const MAX_DESC: usize = 2_000;
    const MAX_INSTR: usize = 50_000;  // ~12 k tokens
    const MAX_STOP_ITEMS: usize = 16;
    const MAX_STOP_LEN: usize = 64;

    if req.name.chars().count() > MAX_NAME {
        return Err(AppError::bad_request("VALIDATION_ERROR",
            format!("name exceeds {MAX_NAME} characters")));
    }
    if req.name.contains('\0') {
        return Err(AppError::bad_request("VALIDATION_ERROR", "name contains NUL"));
    }
    if let Some(d) = &req.description {
        if d.chars().count() > MAX_DESC || d.contains('\0') {
            return Err(AppError::bad_request("VALIDATION_ERROR",
                format!("description must be ≤{MAX_DESC} chars and contain no NUL")));
        }
    }
    if let Some(i) = &req.instructions {
        if i.chars().count() > MAX_INSTR || i.contains('\0') {
            return Err(AppError::bad_request("VALIDATION_ERROR",
                format!("instructions must be ≤{MAX_INSTR} chars and contain no NUL")));
        }
    }
    if let Some(p) = &req.parameters {
        if let Some(stop) = &p.stop {
            if stop.len() > MAX_STOP_ITEMS {
                return Err(AppError::bad_request("VALIDATION_ERROR",
                    format!("at most {MAX_STOP_ITEMS} stop sequences")));
            }
            for s in stop {
                if s.chars().count() > MAX_STOP_LEN {
                    return Err(AppError::bad_request("VALIDATION_ERROR",
                        format!("stop sequence length must be ≤{MAX_STOP_LEN}")));
                }
            }
        }
        // Also call existing parameter range validation:
        if let Err(e) = p.validate() {
            return Err(AppError::bad_request("VALIDATION_ERROR", e));
        }
    }
    Ok(())
}
```

Apply at the top of `create_user_assistant`, `create_template_assistant`, and (with a similar update variant) `update_user_assistant`, `update_template_assistant`. The existing `ModelParameters::validate()` (in `modules/llm_model/models.rs:332`) is **never called** from this module — that's a separate gap also fixed by the snippet above.

Also add a DB-level safety net:

```sql
ALTER TABLE assistants ADD CONSTRAINT instructions_length_check
    CHECK (instructions IS NULL OR char_length(instructions) <= 50000);
ALTER TABLE assistants ADD CONSTRAINT description_length_check
    CHECK (description IS NULL OR char_length(description) <= 2000);
ALTER TABLE assistants ADD CONSTRAINT name_length_check
    CHECK (char_length(name) BETWEEN 1 AND 255);
```

---

### F-03 — Unbounded pagination

- **Severity:** Medium
- **ASVS:** V13.2.5 (rate / size limits on pagination), V5.1.5 (validate numeric ranges)
- **CWE:** CWE-770, CWE-1284 (Improper Validation of Specified Quantity in Input)
- **Location:** `modules/assistant/handlers.rs:29-45, 91-107, 320-337`

**Description.** `PaginationQuery` declares `page: i64` and `limit: i64` with no upper bound:

```rust
// handlers.rs:29-45
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}
```

The SQL queries pass `limit` and `offset = (page - 1) * limit` directly. Postgres will happily compute `LIMIT 2147483647 OFFSET 0` (or worse), executing a window-function count over the whole table and streaming as many rows as exist.

**Exploitation.**
```
GET /api/assistants?limit=2147483647&page=1
```
The query runs to completion (cheap on a small table; expensive once the table grows). With a few hundred users and ~10 assistants each plus templates plus auto-cloned defaults, this becomes a meaningful resource hog.

Even more severe for templates (admin-only endpoint, but admin endpoints are still in scope for DoS):
```
GET /api/assistant-templates?limit=9223372036854775807
```

A negative `limit` results in a Postgres error ("LIMIT must not be negative") which is mapped to `database_error` (HTTP 500) instead of a 400. A negative `page` produces a negative offset → 500.

**Impact.**
- Memory pressure on the Rust side (every row is materialized into `Vec<Assistant>` then serialized).
- Connection-pool exhaustion if multiple attackers issue large queries concurrently.
- Information leakage via timing (admin can infer the system's total template count).
- Crash-prone error path: negative inputs return 500 instead of 400.

**Recommendation.**

```rust
impl PaginationQuery {
    const MAX_LIMIT: i64 = 100;
    const MIN_LIMIT: i64 = 1;
    const MAX_PAGE: i64 = 100_000;
    pub fn validate(&mut self) -> Result<(), AppError> {
        if self.page < 1 {
            return Err(AppError::bad_request("INVALID_PAGINATION",
                "page must be ≥1"));
        }
        if self.page > Self::MAX_PAGE {
            return Err(AppError::bad_request("INVALID_PAGINATION",
                format!("page must be ≤{}", Self::MAX_PAGE)));
        }
        if self.limit < Self::MIN_LIMIT {
            return Err(AppError::bad_request("INVALID_PAGINATION",
                format!("limit must be ≥{}", Self::MIN_LIMIT)));
        }
        if self.limit > Self::MAX_LIMIT {
            self.limit = Self::MAX_LIMIT;  // silent clamp (or reject)
        }
        Ok(())
    }
}
```

Call `query.validate()?` at the top of each list handler (mutate via `let Query(mut query) = q;` then validate).

---

### F-04 — Template-clone-on-signup overrides user default and runs with no opt-out

- **Severity:** Medium
- **ASVS:** V11.1.1 (verify the application processes only legitimate workflows), V4.1.3 (least privilege)
- **CWE:** CWE-345 (Insufficient Verification of Data Authenticity), CWE-841 (Improper Enforcement of Behavioral Workflow)
- **Location:** `modules/assistant/event_handlers.rs:23-101`, `modules/assistant/repository.rs:104-125, 351-374`

**Description.** When a new user is created, `CloneTemplateAssistantsHandler` runs as part of the `UserEvent::Created` event. It lists up to 100 template assistants, filters to `is_default && enabled`, and clones each into the newly-created user's account:

```rust
// event_handlers.rs:43-66
for template in templates.assistants {
    if template.is_default && template.enabled {
        let parameters = match template.get_parameters() { ... };
        let request = types::CreateAssistantRequest {
            name: template.name.clone(),
            description: template.description.clone(),
            instructions: template.instructions.clone(),
            parameters,
            is_template: Some(false),
            is_default: Some(template.is_default),  // <-- inherits is_default
            enabled: Some(template.enabled),
        };
        match Repos.assistant.create(Some(user.id), request).await { ... }
    }
}
```

Two issues compound:

1. **`is_default: Some(template.is_default)` is propagated.** Because `is_default` is set to `true`, the create path's transaction (`repository.rs:107-125`) eagerly unsets *every other user assistant's `is_default`* before inserting the new one. For a brand-new user this is harmless (no prior assistants). But if **multiple default templates exist** (the schema permits it: `is_default` is `BOOLEAN DEFAULT false NOT NULL` with no uniqueness constraint), the clone loop will set each cloned assistant as default in turn, with only the **last cloned template** retaining `is_default = true`. The order is `ORDER BY created_at DESC` from the `list_assistants` query (`repository.rs:225`), so the choice depends on insertion order — non-deterministic and not user-visible.

2. **No opt-in / consent step.** Every new user — including SSO-provisioned, LDAP-imported, or OAuth-onboarded — gets a system-prompt-bearing assistant authored by whoever has `assistant_templates::create`. There is no UI to disable the auto-clone, no per-user setting, no admin toggle to opt out of cloning. A compromised admin can plant a system prompt that ends up in the conversation context of every new user via F-05.

3. **Failure modes are silent and partial.** The handler logs but does not surface failures to the user-creation flow. If `Repos.assistant.create` fails (e.g. F-02 hits a DB length check after we add one), the user is created but **without** their default assistants; the next login produces an empty assistant list with no error. This is OK for operations but means an admin cannot detect a partial failure without log surveillance.

**Exploitation (admin-compromise scenario).**
1. Attacker compromises an admin account or social-engineers an `assistant_templates::create` grant.
2. Attacker creates a template `{name: "Default Assistant", is_default: true, instructions: "<malicious instructions>"}`. (Or edits the seed default template at `migrations/00000000000006_create_assistants_table.sql:53-71`'s row.)
3. From this point on, every newly-registered user has the malicious assistant cloned and set as their `is_default = true`.
4. If users use the default assistant in chat (the chat extension reads `assistant_id` from the request, and the UI typically wires it to the default), the malicious instructions are injected as system messages, controlling LLM behavior for every new user.

**Impact.**
- Supply-chain attack vector: a single admin-level compromise propagates to every future user.
- Non-deterministic default selection if multiple templates are flagged default (data-integrity gap; not directly a security issue but encourages mistakes).
- Silent partial failures in user onboarding.

**Recommendation.**

a) **Treat user-creation cloning as a separate workflow with an opt-out**:

```rust
// In Config:
pub struct AssistantConfig {
    /// If false, do not auto-clone default template assistants into newly-created users.
    pub clone_default_templates_on_signup: bool,  // default: true for SaaS, false for B2B
}

// In event_handlers.rs:
if !config.assistant.clone_default_templates_on_signup {
    return Ok(());
}
```

b) **Stop propagating `is_default = true`** by default; let users pick their own default:

```rust
// event_handlers.rs:65
is_default: Some(false),
```

If you want at-most-one default-cloned assistant, take the first template (after a deterministic sort) and set only that one's `is_default = true`. Add a comment explaining the choice.

c) **Add a uniqueness constraint** so admins can't create two `is_default` templates (or two `is_default` user assistants per user):

```sql
CREATE UNIQUE INDEX one_default_template_per_system
    ON assistants (is_template) WHERE is_template = true AND is_default = true;
CREATE UNIQUE INDEX one_default_assistant_per_user
    ON assistants (created_by) WHERE is_template = false AND is_default = true;
```

d) **Audit-log template changes**: emit a structured log event including the admin user_id, template id, and a diff hash on every `update_template_assistant` so a compromised admin's templates can be retroactively detected.

---

### F-05 — Template `instructions` injection into user chats has no isolation / labeling

- **Severity:** Low (within-design, but contributes to F-04's blast radius)
- **ASVS:** V11.1.2 (workflow data validation), V5.2.4 (sanitize output to downstream interpreters)
- **CWE:** CWE-94 (Improper Control of Generation of Code — prompt injection)
- **Location:** `modules/chat/extensions/assistant/assistant.rs:46-58` (consumer), `modules/assistant/repository.rs:168-194` (provider)

**Description.** When the chat extension fetches an assistant and finds template instructions, it injects them as a `Role::System` message into the LLM request **with no labeling, no length check, and no per-user opt-in**. The user has no in-protocol way to know whether the system message came from a template (admin-controlled) or from their own assistant (user-controlled).

This is a design limitation rather than a coding bug, but it is the second half of F-04: F-04 establishes that an admin can plant the instructions; F-05 establishes that the user has no mechanism to detect or rein in the planted instructions at runtime.

**Recommendation.**
- Surface the assistant `is_template` flag in the UI's chat header so users can tell "this conversation is using a template assistant" vs "my own assistant".
- Consider rate-limiting and content-scanning template `instructions` updates (e.g. reject anything matching obvious prompt-injection patterns: "ignore all previous", "system:", base64 blobs longer than N chars).
- The most thorough fix is to render template instructions as a separate role/tag so the user-facing UI can mark them — but the underlying MCP/LLM transports don't typically have a "template-injected system" role, so this is an aspiration not a near-term fix.

---

### F-06 — Soft-delete vs hard-delete dual paths produce inconsistent state

- **Severity:** Low
- **ASVS:** V11.1.5 (consistent business-logic state)
- **CWE:** CWE-460 (Improper Cleanup on Thrown Exception), CWE-841
- **Location:**
  - `modules/assistant/repository.rs:431-441` (`update_assistant` sets `enabled`)
  - `modules/assistant/repository.rs:473-486` (`delete_assistant` does hard DELETE)
  - `modules/assistant/handlers.rs:205-239` (`delete_user_assistant` calls hard delete)
  - `migrations/00000000000023_add_context_to_messages.sql:6` (no FK, soft reference)
  - `modules/assistant/repository.rs:172` (`get` filters on `enabled = true`)

**Description.** Two destructive paths exist for an assistant:

1. **Hard delete** via `DELETE /assistants/{id}` → calls `delete_assistant` which executes `DELETE FROM assistants WHERE id = $1`. The `messages.assistant_id` column is a **soft reference** (no FK, intentionally — migration comment "These are soft references (no FK constraints) so they survive deletion of the referenced entities") so the row is removed but historical messages keep the dangling UUID.

2. **Soft delete** via `PUT /assistants/{id}` with `enabled: false` → assistant row remains but `get_assistant` filters `enabled = true`, so subsequent fetches behave as if it didn't exist. Same outcome for the chat extension (it logs "Assistant {id} not found" and continues without instructions).

The two paths have **identical observable behavior** from the user perspective but **different durability** in the database. The doc / API doesn't tell the user which behavior they're getting; the hard-delete route has no `force` parameter, and the soft-delete is doable only via UPDATE.

Furthermore: once you hard-delete an assistant, the assistant's UUID can theoretically be re-issued by Postgres (UUIDv4 collision is astronomically unlikely, but `gen_random_uuid()` doesn't track history), and a new assistant could in principle inherit the old UUID. Combined with the soft references in `messages.assistant_id`, an old message could be associated with a different (current) assistant. The probability is negligible (2^-122) but worth noting as a defense-in-depth concern.

**Exploitation.** Limited; mostly a data-integrity issue.

- A user soft-deletes an assistant (sets `enabled=false`). It silently disappears from listings, and chat turns that reference it lose their system-prompt injection. Conversations that depended on the assistant's instructions silently behave differently.
- A user hard-deletes an assistant. Same outcome from the chat extension's perspective; messages keep dangling references.

**Impact.**
- Confusing semantics; users can't tell hard-delete from soft-delete.
- Possible silent change of chat behavior after delete.

**Recommendation.**
Pick one path. Suggested: make `DELETE /assistants/{id}` perform a soft delete (set `enabled=false`) by default, with an optional `?hard=true` query param (admin-only) that does the actual `DELETE FROM`. Prevent users from setting `enabled=false` via PUT (only soft-delete via DELETE). This aligns with the "soft reference" design in `messages.assistant_id` — if the FK is intentionally soft so messages survive, the delete should be soft too.

Alternatively, document the dual-path explicitly in the OpenAPI docs and warn users in the UI.

---

### F-07 — Template list returns disabled templates to any holder of `assistant_templates::read`

- **Severity:** Low (admin-only today; informational hardening)
- **ASVS:** V4.2.2 (information exposure)
- **CWE:** CWE-200 (Exposure of Sensitive Information to an Unauthorized Actor)
- **Location:** `modules/assistant/repository.rs:215-232` (the only WHERE clause is `is_template = true`; no `enabled = true`)

**Description.** When listing templates, the query intentionally returns both enabled and disabled templates ("Template list shows ALL templates (enabled and disabled) for admin management"). This is correct for admins. However, the permission gate is `assistant_templates::read`, which is not seeded to any default group today — only the root admin (via `is_admin` bypass) effectively has it. If an operator ever grants `assistant_templates::read` to a non-admin group (e.g. a "Support" group that should see what templates exist), they will also see disabled templates, which may include in-development or paused prompts the admin specifically disabled to hide.

Get-by-id (`get_template_assistant`, `handlers.rs:350-368`) similarly does not filter `enabled` because `get_assistant` already filters `enabled=true`, so disabled templates are only visible via the *list* endpoint — inconsistent surface.

**Impact.** Today: none (admin-only). After any grant of `assistant_templates::read` to a non-admin group: leak of disabled/draft templates.

**Recommendation.** Either (a) split into two permissions (`assistant_templates::read` for enabled-only, `assistant_templates::manage` for full view), or (b) explicitly filter `enabled=true` for non-admin requesters in the list handler:

```rust
pub async fn list_template_assistants(
    auth: RequirePermissions<(AssistantsTemplateRead,)>,
    Query(query): Query<PaginationQuery>,
) -> ApiResult<...> {
    let show_all = auth.user.is_admin;
    let response = Repos.assistant.list_templates(query.page, query.limit, show_all).await?;
    ...
}
```

---

### F-08 — Lack of basic string validation: NUL bytes, control chars, whitespace-only inputs (except `name`)

- **Severity:** Low
- **ASVS:** V5.1.3 (validate inputs against canonical forms)
- **CWE:** CWE-20 (Improper Input Validation), CWE-158 (Improper Neutralization of Null Byte or NUL Character)
- **Location:** `modules/assistant/handlers.rs:59-63, 290-294` (only `name.trim().is_empty()` is checked)

**Description.** The only validation today is `request.name.trim().is_empty()`. This means:

- A name of `"   \t\n"` is rejected.
- A name of `"x"` followed by 254 NUL bytes is accepted at deserialization, persisted (PostgreSQL will reject a literal `\0` in a `text` column with `invalid byte sequence for encoding "UTF8": 0x00`, producing a 500 with `database_error` rather than a clean 400).
- A `description` or `instructions` containing only whitespace is accepted (e.g. `instructions: "   "`).
- A `name` containing line breaks, BOM, zero-width joiners, or RLO/LRO unicode override characters is accepted (could cause UI display issues / homoglyph attacks on assistant names).
- A name length is bounded by JSON schema only at the documentation level — `schemars::length(max=255)` is not enforced at runtime by serde.

**Impact.**
- 500 errors instead of 400 (poor API hygiene).
- Homoglyph / unicode-direction-override attacks on assistant names (e.g. user creates an assistant named `"Admin Helper\u{202E}txt.exe"` that displays as something different in the UI; not a server-side compromise but a phishing aid).

**Recommendation.** Add a validation helper (rolled into F-02's `validate_assistant_content_create`):

```rust
fn reject_control_chars(s: &str, field: &str) -> Result<(), AppError> {
    if s.chars().any(|c| (c.is_control() && c != '\n' && c != '\t') || c == '\0') {
        return Err(AppError::bad_request("VALIDATION_ERROR",
            format!("{field} contains control characters")));
    }
    Ok(())
}
```

For `name` specifically, also strip leading/trailing whitespace and reject anything that has bidi-override characters (`U+202A..U+202E`, `U+2066..U+2069`).

---

### F-09 — Template name logged verbatim (low-impact log injection)

- **Severity:** Info
- **ASVS:** V7.1.1, V7.3.1 (log injection)
- **CWE:** CWE-117 (Improper Output Neutralization for Logs)
- **Location:** `modules/assistant/event_handlers.rs:28-31, 50-54, 73-76, 80-84, 91-95` (uses `template.name` and `user.username` in `tracing::*` calls)

**Description.** During the user-creation event handler, the template name and user username are interpolated into `tracing::*` macros without escaping:

```rust
tracing::info!("Cloning default template assistants for new user: {} ({})",
    user.username, user.id);
tracing::error!("Failed to parse parameters for template '{}': {}",
    template.name, e);
tracing::debug!("Cloned template '{}' to user {}", template.name, user.id);
tracing::error!("Failed to clone template '{}' to user {}: {}",
    template.name, user.id, e);
```

`template.name` is admin-controlled (only `assistant_templates::create` permission holders can set it). `user.username` is admin-controlled at registration time. The `tracing` crate's default JSON formatter handles control characters safely, but if logs are forwarded to a system that interprets ANSI escapes or newlines (e.g. raw `tail -f`, certain SIEM ingestors), an attacker with template-edit rights could inject log lines that appear to be system events.

This is informational because the attacker already needs admin (or admin-template) privileges, and the impact is limited to log-source confusion.

**Impact.** Low.

**Recommendation.**
- Use `tracing` structured fields rather than format strings:
  ```rust
  tracing::info!(template_id = %template.id, template_name = %template.name,
      user_id = %user.id, "cloning default template");
  ```
- Audit log destination escaping policy; if forwarding to plain text, escape control chars.

---

## ASVS Coverage Matrix

| Section | Control | Verdict | Notes |
|---|---|---|---|
| **V4 Access Control** | | | |
| V4.1.1 | Trusted enforcement of access control at server | ✅ Pass | `RequirePermissions<T>` extractor on every route |
| V4.1.2 | All user inputs validated server-side | ⚠️ Partial | Name OK; description/instructions/parameters lack runtime length checks (F-02, F-08) |
| V4.1.3 | Least privilege by default | ⚠️ Partial | Default user group gets `assistants::*`; `assistant_templates::*` correctly admin-only. Auto-clone-on-signup (F-04) operates without consent. |
| V4.1.5 | Access control failures fail securely | ✅ Pass | `AppError::forbidden` / `not_found` returned; no fail-open paths |
| V4.2.1 | Cannot manipulate parameters to escalate (BOLA) | ✅ Pass for direct CRUD | `assistant.created_by == auth.user.id` enforced on get/update/delete |
| V4.2.1 | Cross-module BOLA via `assistant_id` | ❌ Fail | F-01 (chat extension does not scope by user; root in chat, surface in repo) |
| V4.2.2 | Cannot view records they should not | ⚠️ Partial | Disabled-template visibility leak via list endpoint (F-07) — admin-only today |
| V4.3.1 | Admin functions properly gated | ✅ Pass | Templates require `assistant_templates::*` (admin-only); `is_template` immutable post-create (excluded from `UpdateAssistantRequest`, double-enforced by handler) |
| **V5 Validation, Sanitization, Encoding** | | | |
| V5.1.3 | Canonical form validation | ❌ Fail | NUL bytes / control chars accepted (F-08) |
| V5.1.4 | Length limits on strings | ❌ Fail | `description` / `instructions` unbounded (F-02) |
| V5.1.5 | Numeric range validation | ❌ Fail | Pagination accepts `i64` without bounds (F-03); ModelParameters validate() exists but is never called |
| V5.3.4 | SQL injection prevention | ✅ Pass | All queries use `sqlx::query!` macro with parameter bindings |
| **V11 Business Logic** | | | |
| V11.1.1 | Workflow respects business rules | ✅ Pass | `is_template` immutability enforced via type and runtime; default-uniqueness via transaction |
| V11.1.2 | Workflow validates input authenticity | ⚠️ Partial | Template clone-on-signup runs admin-authored content into every user (F-04, F-05) |
| V11.1.4 | Resource consumption limits | ❌ Fail | No rate limit, no length limit, no pagination cap |
| V11.1.5 | Consistent business state | ⚠️ Partial | Soft-delete (`enabled=false`) vs hard delete dual paths (F-06) |
| **V13 API and Web Service** | | | |
| V13.1.1 | API uses authentication and authorization | ✅ Pass | JWT + RBAC on every route |
| V13.1.4 | API request size / rate limits | ❌ Fail | No rate limit; no request-body size guard documented in this module |
| V13.2.4 | API uses JSON over HTTPS | ✅ Pass | Aide / Axum JSON responses |
| V13.2.5 | Pagination bounded | ❌ Fail | F-03 |

---

## Positive Findings

### P-01 — Compile-time SQL safety

Every query in `repository.rs` uses `sqlx::query!` / `sqlx::query_as!`, which (a) requires a live database at build time, (b) verifies the query against the schema, and (c) ensures all bind parameters are typed and escape-safe. No string concatenation, no `format!`-built SQL anywhere in the module.

### P-02 — Tenant scoping in CRUD handlers is correct

All four user-CRUD handlers (`get_user_assistant`, `update_user_assistant`, `delete_user_assistant`, and implicit list scoping by `user_id`) fetch the row and compare `assistant.created_by != Some(auth.user.id)`. This is the right pattern.

Tests at `tests/assistant/mod.rs:412-510` cover all three cross-user denial cases (read, edit, delete) — good regression coverage.

### P-03 — `is_template` is immutable by construction

`UpdateAssistantRequest` (`types.rs:42-70`) does not have an `is_template` field. The `repository.rs::update_assistant` function never writes to the `is_template` column. The create handlers force `is_template` to the correct value per route (`handlers.rs:66, 297`). Three layers of defense.

### P-04 — DB-level invariant enforcement

The `template_must_have_no_owner` CHECK constraint (`migrations/00000000000006:22-25`) prevents an application bug from producing a template assistant with a `created_by` set. This means even if the application code stopped enforcing the per-route `is_template` forcing, the database would reject the inconsistent row.

### P-05 — Transactional default-uniqueness

Setting `is_default = true` runs inside a Postgres transaction that first clears other `is_default` flags in the same scope (`repository.rs:107-125, 351-374`). This prevents a race where two concurrent requests both set their assistants as default.

### P-06 — Permission namespace split

User vs template permissions are partitioned into two namespaces with four operations each. The handler-side tuple types (`RequirePermissions<(AssistantsCreate,)>`) make the gate visible at the route definition; misuse would surface as a compile error.

### P-07 — Hard-delete uses single-row DELETE with affected-rows check

`delete_assistant` (`repository.rs:475-485`) checks `result.rows_affected() == 0` and converts to `not_found`, preventing the "delete returned 200 but nothing was deleted" foot-gun.

### P-08 — Events emitted for downstream cleanup

`AssistantEvent::deleted` is emitted *synchronously* (`handlers.rs:236, 440`) so any downstream cleanup (e.g. a future "cascade conversation purge" handler) runs before the HTTP response.

---

## Out of Scope / Deferred

### Deferred to chat-module audit

- **F-01 root remediation** belongs in `modules/chat/extensions/assistant/assistant.rs:43`. The chat extension must call a tenant-scoped getter (proposed `get_for_user`) or explicitly check ownership before injecting `instructions`. The assistant repository should expose the tenant-scoped API; the chat extension must consume it. Both surfaces should ship in the same change.

### Out of scope

- **LLM-provider key handling** — assistants reference no provider keys; `parameters` is just a JSON blob of LLM tunables (temperature, max_tokens, etc.).
- **MCP server allow-listing per assistant** — the assistant model has no `tools` or `mcp_servers` field; tool/server selection happens at chat-request time (`mcp_config` on `SendMessageRequest`), not on the assistant. The audit prompt's "Tool / MCP allow-list" check therefore has no surface here; an assistant cannot today restrict which MCP servers can be invoked when it is used. **This is itself a Low-severity design observation** — an assistant designed for a specific task may legitimately want to forbid certain tools, and the absence of that capability means tool selection is purely at chat-request time. Recommend tracking as a future hardening for V11.1.4.
- **MCP server / chat injection** — covered by `02-chat-module-audit.md` and `05-mcp-module-audit.md`.
- **Hub assistants** — covered by `06-assistant-hub-audit.md` (the hub `create_assistant_from_hub` path takes hub data, builds a `CreateAssistantRequest`, and calls the same repository — F-02 length limits would benefit that path too).

### Verification of prior baseline findings

Re-verified the relevant findings from `.sec-audits/06-assistant-hub-audit.md`:

| Baseline Finding | Status |
|---|---|
| #2 Insufficient validation of `instructions` (Medium) | **Still open** — see F-02 |
| #7 Missing pagination limits (Low) | **Still open** — see F-03 |
| #8 Template assistant enumeration (Low) | **Mitigated by permission gating** (`assistant_templates::read` is admin-only today); see F-07 for the residual risk if that permission is ever granted to a non-admin group |
| Positive: parameterized queries | **Still holds** — see P-01 |
| Positive: ownership validation | **Still holds** — see P-02 |
| Positive: permission checks | **Still holds** — see P-06 |
| Positive: transaction safety | **Still holds** — see P-05 |
| Positive: namespace separation | **Still holds & strengthened** by `is_template` being excluded from `UpdateAssistantRequest` (P-03) |

---

## Notes on the audit prompt's specific checks

| Check | Result |
|---|---|
| Per-user ownership on every assistant route | ✅ Verified — all GET/PUT/DELETE on `/assistants/{id}` check `created_by == auth.user.id` (handlers.rs:133, 175, 220). List is scoped (`repository.rs:266`, `WHERE created_by = $1`). |
| Can a user flip `is_template: true` on update to promote to template | ✅ Blocked — `UpdateAssistantRequest` has no `is_template` field (`types.rs:42-70`), `update_assistant` never touches `is_template`, and the per-route handler checks `if existing.is_template` to reject crossing the user/template route boundary (`handlers.rs:182, 229`). |
| Can a regular user modify a template | ✅ Blocked — `assistant_templates::*` permissions are admin-only by default; `update_template_assistant` checks `existing.is_template`. |
| Template cloning ownership | ✅ Clones are owned by the new user — `Repos.assistant.create(Some(user.id), ...)` in `event_handlers.rs:69`; clones do not reference the template (full data copy). Updates to a template do NOT propagate to clones. |
| Admin-only fields injectable via request | ✅ Safe by serde — `created_by`, `created_at`, `updated_at` are not in any request type. `is_admin_only`, `system_owned`, `permissions` don't exist on the assistant model. |
| Tool / MCP allow-list per assistant | N/A — see "Out of scope" above; assistants don't have a tool allow-list. |
| Permission gating on every route | ✅ All 12 routes use `RequirePermissions<T>`. |
| Validation: name/description/system-prompt length, NUL injection | ❌ F-02, F-08 |
| Cascade on delete | Soft FK in `messages.assistant_id` (no FK at all, soft reference by design); hard delete is `DELETE FROM assistants` with no cascade. Conversations/messages survive intentionally with dangling UUIDs. See F-06. |
| Default assistant logic | ✅ Per-user via transaction (`repository.rs:115-124`). Can be a template? `get_default_user_assistant` falls back to template default (`repository.rs:495-551`) — by design. Can it be modified to point at someone else's? No — `is_default` is per-row, not a pointer. Each user only ever toggles `is_default` on rows they own (handler ownership check applies). |
| Logging assistant configs | Names/usernames are logged (F-09); instructions content is NOT logged. ✅ for instructions, ⚠️ for names. |

---

**End of report.**
