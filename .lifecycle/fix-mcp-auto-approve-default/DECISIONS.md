# DECISIONS — fix-mcp-auto-approve-default

Every human/product input this implementation needs, resolved up front. No item is
left unresolved; implementation runs without a further stop.

Two decisions were escalated to the human as explicit option pickers before planning
(DEC-5, DEC-12); their chosen options are recorded verbatim below.

---

### DEC-1: Should the deployment's default approval mode be a fixed compile-time constant or an admin-configurable settings row?

**Resolution:** Fixed compile-time constant — `ApprovalMode::default()` in
`chat_extension/approval/models.rs`. NOT promoted to a settings table.

**Basis:** user + convention. The lifecycle's configurable-settings rule defaults an
operational tunable to an admin-configurable singleton row, and that is the right
long-term shape. It is deliberately NOT done here for three reasons, in order of
weight: (1) the human's explicit constraint on this change is **no new migration file,
no seed SQL, no backfill** — a settings row requires a migration by construction;
(2) this is a *security posture* boundary (whether third-party MCP tool calls run
without a human in the loop), which is exactly the category the rule permits to stay a
constant rather than something an operator can weaken from a web form; (3) the value
already IS operator-selectable at the only layer that matters for this deployment
model — the `deploy-schedule` branch compiles `AutoApprove`, `khoi`/`main` compile
`ManualApprove`. The constant is structured as the enum's `#[default]` (one named
place, not an inline magic value), so promoting it to a settings row later is a
mechanical change with no rewrite. Recorded so the choice is explicit, not an omission.

---

### DEC-2: Which single place spells the deployment default, given five disagreeing sources today?

**Resolution:** `ApprovalMode`'s `#[default]` variant. Everything else derives from it:
both `get_approval_mode()` unwraps, `mcp.rs`'s no-row branch, and
`settings/repository.rs`'s `DEFAULT_APPROVAL_MODE`.

**Basis:** codebase. `#[derive(Default)]` + `#[default] ManualApprove` already exist at
`approval/models.rs:13-21` and are currently consumed by nothing (verified: no
`unwrap_or_default()` on an `ApprovalMode`, no `..Default::default()` on any struct
holding one). Promoting the existing-but-dead impl adds no new type, no new constant,
and no new import — and it collapses the khoi↔deploy divergence from four lines in four
files to one line in one file, which is what makes PR-2 a clean cherry-pick.

---

### DEC-3: `DEFAULT_APPROVAL_MODE` is a `const &str`, but `ApprovalMode::default().to_string()` is not const-evaluable. What shape replaces it?

**Resolution:** A private `fn default_approval_mode() -> String` in
`settings/repository.rs` returning `ApprovalMode::default().to_string()`, replacing the
`const`. Its two use sites (`:104`, `:131`) already build owned `String`s
(`.to_string()`), so they are unchanged apart from the call.

**Basis:** convention — the file's existing helper `fn chrono_from_ts(...)`
(`settings/repository.rs:361`) is the same shape (a small private free function at
module scope). A `LazyLock<String>` was rejected: it adds a static and a dependency for
a value produced by a 3-arm match. Resolves the PLAN_AUDIT ITEM-3 CONCERN.

---

### DEC-4: How is "the caller did not specify an approval mode" represented on the wire, and what does the server do with it?

**Resolution:** `approval_mode: Option<ApprovalMode>` with `#[serde(default)]` — absent
⇒ `None`. The server resolves `None` with an INLINE `COALESCE` inside the existing
upsert statement: `COALESCE($n, '<ApprovalMode::default()>')` on the VALUES arm (insert)
and `COALESCE($n, <table>.approval_mode)` on the `DO UPDATE` arm (preserve). Absent
therefore means "use the server default" on create and "do not touch" on update.

**Basis:** codebase. The same statement already does exactly this for
`auto_approved_tools` (`approval/repository.rs:66-70,82,86`), and the frontend already
documents the contract in a comment: *"backend COALESCE preserves DB value otherwise"*
(`McpComposer.store.ts:410`). Note `#[serde(default)]` on an `Option` yields `None`, NOT
`Some(default)` — that distinction is load-bearing and is pinned by TEST-6/TEST-7.

---

### DEC-5: Should already-clobbered `mcp_settings` rows on the live instance be backfilled?

**Resolution:** **No.** Code fix only. No migration, no seed, no backfill SQL — not in
the repo, not in a PR body, not handed over out-of-band. No statement in this change
may touch any row other than the single one being upserted; no mass `UPDATE`/`DELETE`
on any branch. Existing conversations keep whatever mode they have; a new conversation
gets the correct default.

**Basis:** user — chosen explicitly from an option picker ("Code fix only", over
"Verbatim SQL in PR body" and "Blanket flip"), and re-affirmed afterwards with the
added constraint that the SQL change stay an inline COALESCE inside the two existing
upserts and never mutate pre-existing rows. TEST-12 is the direct proof of the
never-mutate property.

---

### DEC-6: How does the client learn the server's default — synthesize a `defaults` object when the user has no row, or add a sibling field?

**Resolution:** Add a sibling scalar `default_approval_mode: ApprovalMode` to
`UserMcpDefaultsGetResponse`. `defaults` stays `Option` and stays `null` when unset.

**Basis:** codebase. Synthesizing would have to fabricate `id`, `user_id`,
`created_at`, `updated_at` (`UserMcpDefaultsResponse` requires all four); it would flip
`McpInitializer.tsx:39`'s `if (userDefaults)` from the select-all-servers branch into
`applyUserDefaultsToPending`, a behaviour change unrelated to this bug; and it would
break the existing assertion at `mcp_defaults_test.rs:64`. A sibling field is purely
additive and mirrors the existing `{ settings: Option<..> }` / `{ defaults: Option<..> }`
response shape used throughout this module.

---

### DEC-7: What does the frontend use as the approval mode before `GET /api/mcp/defaults` has resolved, or if it fails?

**Resolution:** A single named constant `FALLBACK_APPROVAL_MODE = 'manual_approve'` in
the new `approvalDefaults.ts`, used ONLY as the pre-fetch / fetch-failed value, and
never as a value that gets PUT (the PUT sites omit the field instead — DEC-8).

**Basis:** convention — fail safe. If the server default is genuinely unknown, showing
and assuming the *more restrictive* mode is the correct posture: the worst case is the
user is asked to approve a tool that would have auto-run, which is recoverable; the
inverse silently runs a third-party tool without consent. `loadUserDefaults` already
swallows its error and sets `userDefaultsLoaded = true` (`McpComposer.store.ts:1010-1015`),
so this path is reachable and needs a defined value.

---

### DEC-8: On the initial auto-persist, should the frontend send the server default it just learned, or omit the field?

**Resolution:** **Omit** `approval_mode` when the local config has no explicit mode.
The server (DEC-4) then applies its own default on insert / preserves on update. The
frontend still sends `disabled_servers` — that server-list snapshot is the reason the
turn-1 write exists at all and must not be lost.

**Basis:** convention + defence in depth. Omitting makes the SERVER authoritative
rather than trusting a client round-trip: a stale cached bundle, a failed defaults
fetch, or a third-party API client can no longer downgrade a conversation by echoing a
value it guessed. It also mirrors how the same PUT already treats `auto_approved_tools`
(sent only when `updateAutoApproved` is set — `McpComposer.store.ts:411`).

---

### DEC-9: Does the "omit when unset" rule apply to all three PUT sites in the store?

**Resolution:** No — it applies to `saveConversationConfig` (`:409`) and
`saveUserDefaults` (`:1040`) only. `saveProjectConfig` (`:485`) targets
`PUT /projects/{id}/mcp-settings`, whose request type `ProjectMcpSettingsRequest`
(`project_extension/models.rs:25`) keeps `approval_mode` REQUIRED; it therefore keeps
sending a value, but sourced from the server default instead of the `'manual_approve'`
literal.

**Basis:** codebase. Project MCP settings are only ever written by an explicit user
action in the project modal (there is no turn-1 auto-persist for a project), so the
clobber this bug is about cannot occur there. Widening the project request type too
would be scope the bug does not need, and it would add a third schema delta to the
regen. The project READ path still gets the corrected default via `get_or_default`
(ITEM-3). Resolves the PLAN_AUDIT ITEM-11 CONCERN.

---

### DEC-10: The DB column default is `manual_approve` on BOTH branches and disagrees with the deploy code default. Change it?

**Resolution:** No. Leave `202607140180_mcp_schema.sql:56,132` untouched; record in a
code comment why it is unreachable.

**Basis:** codebase + user constraint. Every `INSERT` in the tree that writes these
tables names `approval_mode` explicitly (`approval/repository.rs:79-82`,
`defaults/repository.rs`, `settings/repository.rs:155-159,196-200,305-309,330-334`,
`project_extension/extension.rs:81`), so the column default can never be applied.
Changing it would require a migration, which DEC-5's constraint forbids, in exchange
for zero behavioural difference.

---

### DEC-11: How does the e2e mint a conversation for the literal repro without a working LLM provider?

**Resolution:** Use the existing `createConversationWithModel` helper
(`e2e/chat/helpers/chat-helpers.ts:282-315`), which fills the composer, clicks Send,
and waits only for the `/chat/{id}` URL — NOT for an assistant reply. The conversation
row and the frontend's `onMessageSent` turn-1 auto-persist both happen on the send,
independent of whether the upstream LLM call subsequently fails.

**Basis:** codebase. This is how the existing chat e2e specs obtain a conversation id
with the fake-key OpenAI provider created by their `beforeEach`
(`mcp-config-modal.spec.ts:28-36`, `mcp-chip-row-persistence.spec.ts:28-37`). It keeps
TEST-20 the LITERAL reported repro (a real send on a real new chat) rather than an
API-created conversation, per rule B9.

---

### DEC-12: How much verification before the PRs?

**Resolution:** Targeted tests (unit + integration + e2e per TESTS.md) **plus** a local
docker review stack that reproduces the exact symptom end-to-end on the deploy default,
with a khoi-image negative control, then is torn down.

**Basis:** user — chosen explicitly from an option picker ("Add a local docker review
stack"). Every docker invocation is wrapped as a single `sg docker -c "..."` call; the
stack binds a free `ZIEE_WEB_PORT` (never the live 18130) under its own
`COMPOSE_PROJECT_NAME`, and its containers, images and volumes are removed afterwards.

---

### DEC-13: Which branch carries which default, and how is the divergence kept to one line?

**Resolution:** `fix/mcp-auto-approve-default` (→ PR-1 into `khoi`) keeps
`#[default] ManualApprove`. `fix/mcp-auto-approve-default-deploy` (→ PR-2 into
`deploy-schedule`, **never** `deploy`) carries `#[default] AutoApprove` as the ONLY
intentional difference; the deploy branch's three other current overrides
(`approval/models.rs:122`, `defaults/models.rs:105`, `mcp.rs:2603`) and its
`settings/repository.rs:21` override all disappear because they now derive from the
enum.

**Basis:** user (task brief: two branches, two PRs, deploy freeze) + DEC-2. The
cherry-pick therefore has exactly one expected conflict, at a single line, which is
also the reason the collapse in DEC-2 is worth doing as part of this fix rather than
later.
