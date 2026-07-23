# INFRA_INTEGRATION — fix-mcp-auto-approve-default

The three mandatory per-item walks (user-experience, infrastructure-integration,
entity-lifecycle), done while implementing.

## 1. User-experience walk

**How a real user encounters this.** They open a new chat on a deployment whose MCP
tools are meant to run without prompting, type a question, and the model answers using
`query_rag` — no prompt. They ask a follow-up. Today they get an approval dialog for
the same tool they were never asked about a minute ago. There is no setting they
touched to cause it and no setting they can find to stop it — the config modal says
"Manual Approve", which is not what they configured and not what turn 1 did. The
experience is "the permission system is broken/random".

After the fix the modal tells the truth from the first render, and turn 2 behaves like
turn 1. Nothing new to click; one existing control stops lying and one silent write
stops overriding the deployment.

**What the user can still do.** Choosing Manual in the modal still sticks (TEST-13,
TEST-12). Auto-approving a single tool under Manual still sticks (TEST-14). Those are
the two paths a user could reasonably fear this change breaks, so both are pinned.

**Failure mode kept safe.** If `GET /api/mcp/defaults` fails, the client shows
`manual_approve` (DEC-7) — the user is asked about a tool that would have auto-run.
The inverse (guessing auto-approve and silently running a third-party tool) is not
recoverable, so the fallback is deliberately the restrictive one.

## 2. Infrastructure-integration walk

Every subsystem the changed code touches, and what each one specifically required:

| Subsystem | Finding |
|---|---|
| **MCP approval gate** (`mcp.rs::after_llm_call`) | The primary consumer. Extracted `resolve_approval`; the three-branch precedence is unchanged, branch 3 now reads `ApprovalMode::default()`. |
| **`run_js` inner-tool gate** (`js_tool/approval.rs`, `executor.rs`) | NOT obvious from the plan: `execute_run_js_call` (`mcp.rs:682`) is handed the SAME resolved `approval_mode`. So the extraction had to stay a single source for both gates — had I resolved separately for the MCP path only, `run_js`'s inner tool calls would have diverged from the outer ones. Verified it reads the value through the same variable. |
| **Workflow tool dispatcher** (`workflow/dispatch.rs:1162`) | Calls `get_user_defaults` for a standalone/scheduled run — but uses ONLY `get_disabled_servers()`, never `get_approval_mode()`. Unaffected. Checked rather than assumed, because a scheduled run has no user to answer a prompt and a default flip there would be a security change. |
| **Project MCP settings** (`project_extension/`) | Reads via `Repos.mcp_settings.get_or_default`, which routes through the corrected default (ITEM-3) — so the project surface inherits the fix for free. Its PUT keeps `approval_mode` REQUIRED (DEC-9): there is no auto-persist on the project path, so no clobber to close, and widening it would add a third schema delta. |
| **Built-in server bypass** (`is_builtin_server_id`) | Untouched, per the task's explicit non-goal. Built-ins bypass approval before the mode is consulted, so this change cannot affect them either way. |
| **Unattended / scheduled runs** (`unattended_tool_allowed`) | Sits AFTER the `needs_approval` decision. On a deploy build, branch 3 already returned `AutoApprove`, so the set of tools reaching the unattended allow-list is unchanged by this fix; what changes is only that turn 2 now matches turn 1. |
| **Realtime sync** | `update_mcp_settings` still emits `SyncEntity::Conversation`; `update_mcp_defaults` still emits `SyncEntity::McpDefaults`. Notify-only payloads, so a changed field needs no sync change — the client refetches and picks up `default_approval_mode` on the same GET. |
| **Permissions** | No new permission. The two endpoints keep `conversations::read` / `conversations::edit`; TEST-16 re-pins both 403 paths after the request-type change. A10 (restricted-user e2e) therefore does not apply. |
| **OpenAPI / TS codegen** | Three schema deltas → `openapi.json` + `types.ts` for BOTH workspaces. Verified the large `openapi.json` diff is key-order churn by diffing sorted files: the only content deltas are the two `required`-removals + `anyOf/null`, and the new `default_approval_mode` property. |
| **Gallery mock-API cassette** | Not anticipated in the plan. `Mcp.getDefaults` was recorded as `{}`; a newly-REQUIRED response field broke `tsc` on `crawl.generated.ts`. Fixed at the fixture (recorded a realistic response) and moved the endpoint into the generator's existing, documented `LOOSE` set — the set that exists precisely for "a union/enum the structural JSON doesn't satisfy exactly". Every other enum-carrying recorded endpoint is already there; this is following the convention, not bending the harness. |
| **Desktop UI overrides (R2-3)** | `src-app/desktop/ui/src/modules/` has NO `mcp` module — checked its full contents. Desktop's only exposure is the regenerated `api-client/types.ts` + `openapi.json`, both produced by the desktop binary (never `cp`-ed from the server spec). No hand-written desktop counterpart to keep in sync. |

## 3. Entity-lifecycle walk

The only entity this change touches is the **`mcp_settings` row** (conversation scope)
and the **`user_mcp_defaults` row** (user scope). Neither is a new entity; neither
gains a new surface.

| Event | Conversation row | User-defaults row |
|---|---|---|
| **ADD** | Created by the client's turn-1 auto-persist, or by an explicit modal save. Now takes the server default when the caller didn't choose (TEST-11 / e2e TEST-20). | Created by an explicit "Save as Default", or as a side effect of a chip removal. Now takes the server default when the caller didn't choose (TEST-15 / e2e TEST-21). |
| **MUTATE (explicit)** | Modal save sets the mode; persisted verbatim (TEST-13). | Same. |
| **MUTATE (incidental)** | A save that omits the mode leaves it untouched (TEST-12) — the property the whole fix rests on. | Same (TEST-15). |
| **REMOVE / access-loss** | Cascade-deleted with the conversation (existing FK), unchanged by this diff. | Per-user singleton; unchanged. |
| **Cross-device (sync)** | `SyncEntity::Conversation` / `McpDefaults` notify-only → the client refetches through the same GET, so it sees the same `default_approval_mode` the local path did. No separate handler to keep in step. |

There is no local-vs-sync divergence risk here (the trap FB-23 describes) because the
client holds no derived per-entity view of this state: every read is a refetch of the
same endpoint, and the only cached value (`serverDefaultApprovalMode`) is refreshed by
the existing `sync:mcp_defaults` / `sync:reconnect` subscription that already calls
`loadUserDefaults`.

## Pre-existing breakage found (NOT caused by this diff, NOT fixed here)

Recorded so a later reader doesn't attribute them to this change. Both were verified
against a clean `origin/khoi` tree.

1. **`server/tests/path_resolution.rs` does not compile** — `E0507: cannot move out of
   dereference of Config` at `:153`, from `ziee_core::config::EmbeddedPostgreSqlConfig`
   not being `Copy`. Untouched by this diff. It breaks `cargo check --all-targets`;
   `--lib` and `--test integration_tests` are unaffected, so the scoped commands in
   TEST_RESULTS are used.
2. **`npm run check:testid-registry` fails** — `sdk/packages/kit/src/testIds.generated.ts`
   (inside the `sdk` SUBMODULE) is stale against ziee's `src/`: it is missing the
   split-chat pane ids and still carries notification ids. Proven pre-existing by
   stashing every UI source change in this diff and re-running the check on the clean
   tree — it still failed. Regenerating would require a commit to a different repo
   (the sdk submodule), which is outside this branch and would be editing shared
   infrastructure to route around an unrelated failure (rule B3). Left alone; recorded
   in TEST_RESULTS with the evidence.
