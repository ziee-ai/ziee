# Audit — Round 4 (Final Sweep)

Branch worktree: `/home/pbya/projects/ziee-chat-feat-project-improvements`

This round surfaced **3 confirmed findings**, all in the lower
severity bands: 1 medium (missing test for a new SQL path), 1 low
(missing test for collision-suffix logic), and 1 nit (stale doc
comment). There are **no high-severity findings** and **no behavioral
bugs** — every confirmed item is either a test-coverage gap on
newly-added code or a documentation inaccuracy. The underlying runtime
behavior is correct in all three cases; the load-bearing security
invariant (per-user project ownership + attach-handler tenant guard)
holds today.

## Summary

| ID | Severity | Category | File | Title |
|---|---|---|---|---|
| r4-final-sweep-01 | medium | missing-test | `src-app/server/tests/code_sandbox/tier2_repository.rs` | New project-files-in-sandbox SQL path (`project_refs` UNION) has no integration test |
| r4-final-sweep-02 | low | missing-test | `src-app/server/src/modules/code_sandbox/sandbox.rs:604-641` | Duplicate-filename collision-suffixing in `build_bwrap_argv` (+ `read_file` `AMBIGUOUS_FILENAME` branch) is untested |
| r4-final-sweep-03 | nit | doc | `src-app/server/src/modules/memory_mcp/repository.rs:20-21` | Stale doc comment: `upsert_builtin_server` claims immutability is enforced by gating on the `is_built_in` column |

**Severity counts:** high 0 · medium 1 · low 1 · nit 1

---

## r4-final-sweep-01 — New project-files-in-sandbox SQL path has no integration test

- **Severity:** medium · **Category:** missing-test
- **File:** `src-app/server/tests/code_sandbox/tier2_repository.rs`
- **Source under test:** `src-app/server/src/modules/code_sandbox/repository.rs` (`get_conversation_files`, lines 104-124)

### Evidence

`git diff main..HEAD` on `code_sandbox/repository.rs` (commit
`09b81114`) confirms the `project_refs` CTE + `UNION` is **new** on
this branch; `get_conversation_files` now reaches project knowledge
files. The path deliberately omits the `f.user_id` predicate (comment
at lines 108-113) and leans on a cross-tenant invariant that is verified
real:

- `projects.user_id` is `NOT NULL` / per-user — migration
  `00000000000051` line 23.
- the attach handler 404s a foreign file before inserting into
  `project_files` —
  `src-app/server/src/modules/file/project_extension/handlers.rs:78`
  (`if file.user_id != auth.user.id`).

Coverage gap verified: `get_conversation_files` is referenced **only**
in `tier2_repository.rs` across all tests; that file is untouched by
this branch (empty diff vs `main`). Existing tests cover the attachment
path only (nonexistent conv, missing user_id, `get_file_by_id`
foreign-user denial, malformed-UUID JSONB filter) — none inserts a
`project_files` / `project_conversations` row. No test elsewhere
(`project/`, `files_mcp`, `agentic_chat`) exercises the sandbox
project-file SQL path; `project/injection_test.rs` covers the LLM-chat
injection path, not `get_conversation_files`.

Medium (not high): a coverage gap, not a live exploit — the invariant
holds today.

### Fix

Add two Tier-2 tests to `tier2_repository.rs` (already registered via
`mod.rs` `mod tier2_repository;`), mirroring the existing direct-SQL
setup in `get_conversation_files_filters_malformed_uuid_in_jsonb` and
`get_file_by_id_denies_foreign_user`.

**Positive test — project knowledge file surfaces in the sandbox file set:**
1. INSERT a user; a project (`INSERT INTO projects (id, user_id, name, ...)`);
   a conversation owned by that user; a `files` row owned by that user;
   `INSERT INTO project_files (project_id, file_id)`;
   `INSERT INTO project_conversations (conversation_id, project_id)`.
2. Assert `repo.get_conversation_files(conv)` returns a `Vec` containing
   that `file_id`.
3. To pin the `SELECT DISTINCT` dedup + the new `ORDER BY f.created_at,
   f.id`: attach the **same** file as both a project file and a chat
   attachment (add to `message_contents` like the malformed-UUID test)
   and assert it appears exactly once; or attach two project files
   sharing a `created_at` and assert stable ordering by `f.id`.

**Negative test — pin the no-user_id-predicate assumption:**
1. Create user A (project + conversation owner) and user B.
2. INSERT a `files` row owned by B directly via SQL (bypassing the
   attach-handler 404 guard), then `INSERT INTO project_files` for A's
   project pointing at B's file (simulating a future bug that breaks the
   handler invariant).
3. Assert `get_conversation_files(conv)` **still** returns B's file
   (the SQL has no `user_id` filter) — documenting that the only thing
   protecting cross-tenant leakage here is the attach-handler boundary,
   NOT the query. This makes the security contract explicit and will
   flag any future change to the invariant. (Alternatively, if the team
   decides the query should defend in depth, add `AND f.user_id =
   (SELECT user_id FROM conversations WHERE id = $1)` and assert B's file
   is excluded — a design choice beyond the test gap; the minimal fix is
   the regression-pinning tests above.)

Use `ziee::code_sandbox::CodeSandboxRepository` (already imported) and
the existing `repo(&server)` helper; follow the FK insert ordering
already demonstrated (conversation row → branch → `UPDATE
active_branch_id`) only if also exercising the attachment dedup path.

---

## r4-final-sweep-02 — Collision-suffixing in `build_bwrap_argv` (+ `read_file` ambiguity branch) is untested

- **Severity:** low · **Category:** missing-test
- **Files:** `src-app/server/src/modules/code_sandbox/sandbox.rs:604-641`; `src-app/server/src/modules/code_sandbox/tools/files.rs:178-209`

### Evidence

Reproduced from current worktree code:

1. `sandbox.rs:612-643` — collision-safe mount logic: a `seen
   HashSet<String>` plus a `for i in 2..` loop that suffixes `" (2)"`,
   `" (3)"`, … before the extension via `file_stem`/`extension`
   arithmetic, mapping dest to `/home/sandboxuser/{dest_name}`.
2. `tools/files.rs:178-209` — matching `AMBIGUOUS_FILENAME` branch in
   `load_file_content`: when `ctx.files` has >1 same-named attachment it
   computes `suffixed_examples` and returns `BAD_REQUEST` pointing the
   model at `execute_command` + `cat`.

Both added in commit `09b81114` ("duplicate-safe sandbox mount");
`git show 09b81114` over both files yields **zero** added
`#[test]`/`#[tokio::test]` lines. Every unit test in both `#[cfg(test)]`
modules builds `ctx.files` as an **empty** `Vec`: `fake_ctx()`
(`sandbox.rs:1037-1044`) uses
`Arc::new(Vec::<ConversationFile>::new())`, and `ctx_for()`
(`files.rs:552-559`) does the same — so neither the dedup loop nor the
ambiguity branch is ever exercised.

The cited `mcp_argv_emits_extra_ro_binds` (`sandbox.rs:1546`) exercises
`build_mcp_sandbox_argv` (the MCP path) with a single pre-built bind
tuple and never touches `build_bwrap_argv`'s dedup logic. The
superficially-similar `test_read_file_ambiguous_name_errors` in
`tests/files_mcp/mod.rs` is a **different** module (the files_mcp MCP
server's id/name disambiguation), not this `code_sandbox` branch.

The `for i in 2..` suffix arithmetic and stem/ext split are exactly the
kind of logic that silently regresses (off-by-one, wrong split for
dotless / multi-dot names) with no test to catch it. Low is correct: a
coverage gap, not a behavior bug.

### Fix

Two in-source unit tests (no DB; both paths are pure given an in-memory
ctx):

- **`sandbox.rs` `#[cfg(test)] mod tests`:** build a `SandboxContext`
  whose `files` Arc holds two `ConversationFile`s with filename
  `"data.csv"` (distinct `file_id`s), call `build_bwrap_argv`, assert
  the argv contains a `--ro-bind` window mapping to
  `/home/sandboxuser/data.csv` **and** another to
  `/home/sandboxuser/data (2).csv`. Add a dotless case (two `"Makefile"`
  → `Makefile` and `"Makefile (2)"`) to pin the no-extension branch.
  Construct the ctx inline since `fake_ctx()` hard-codes an empty file
  list.
- **`tools/files.rs` `#[cfg(test)] mod tests`:** add a `#[tokio::test]`
  that builds a ctx with two `ConversationFile`s sharing filename and
  the same `user_id` (matching `ctx.user_id`), where the file is **not**
  present in the workspace dir so `load_file_content` falls through to
  the NotFound attachment branch; call `read_file(&ctx, "data.csv",
  None, None)` and assert the `AppError` is `BAD_REQUEST` with
  code/message containing `AMBIGUOUS_FILENAME` and the suffixed hint
  `"data (2).csv"` plus `execute_command`/`cat`.

`ConversationFile` fields: `file_id: Uuid`, `filename: String`,
`user_id: Uuid`, `mime_type: Option<String>`, `created_at:
DateTime<Utc>` (`models.rs:9`). Keep hand-narrow ~80col style; do not
rustfmt.

---

## r4-final-sweep-03 — Stale doc comment on `upsert_builtin_server`

- **Severity:** nit · **Category:** doc
- **File:** `src-app/server/src/modules/memory_mcp/repository.rs:20-21`

### Evidence

`memory_mcp/repository.rs:20-21` states built-ins are immutable because
"`update_system_mcp_server` rejects any modification of an `is_built_in`
row." That mechanism description is inaccurate.
`update_system_mcp_server` (`mcp/repository.rs:1596-1604`) gates on the
deterministic zero-config ids only — `is_zero_config_builtin =
existing.id == files_mcp_server_id() || existing.id ==
memory_mcp_server_id()` — and explicitly **not** on the `is_built_in`
column. The in-line comment above that guard (lines 1586-1595) says so:
admin-configurable built-ins (filesystem/fetch/browser/git via migration
25, plus code_sandbox) also carry `is_built_in=true` and must stay
editable; gating on `is_built_in` "regressed editing them."

`test_update_configurable_builtin_is_editable` (`tests/mcp/mod.rs:717-771`)
pins this: it PUTs an edit to the seeded filesystem row
(`is_built_in=true`) and asserts HTTP 200 with the edited description
applied. The memory server is still correctly rejected because its id is
one of the two zero-config ids, so runtime behavior is fine — only the
doc comment's stated reason is wrong and could mislead a maintainer into
thinking any `is_built_in` row is immutable. Nit/doc: no behavioral
impact.

### Fix

Reword the doc comment so the stated reason matches the actual id-based
guard. Replace:

```rust
/// Built-ins are immutable via the API: `update_system_mcp_server`
/// rejects any modification of an `is_built_in` row, so the
```

with:

```rust
/// The memory server is immutable via the API: `update_system_mcp_server`
/// rejects modification of the two zero-config built-in ids
/// (files/memory), gated on the deterministic id — NOT the `is_built_in`
/// column (admin-configurable built-ins like filesystem/code_sandbox
/// share that column but stay editable). So the
```

The remainder of the comment ("ON CONFLICT DO UPDATE clause only
re-asserts identity columns …") is unchanged and remains accurate. Keep
lines wrapped at ~80 cols; do not rustfmt.

---

## Convergence

The audit has **not** fully converged: one **medium** finding remains
(r4-final-sweep-01, a missing integration test for a new SQL path with a
baked-in cross-tenant ownership assumption). The remaining two findings
are a low-severity test gap and a documentation nit. No high-severity
and no behavioral findings remain.
