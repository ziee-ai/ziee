# Phase-5 walks — UX / infrastructure-integration / entity-lifecycle

## 1. User-experience walk

The "user" of these two contracts is a language model, and the experience is the
refusal text it reads and acts on.

| before | after |
|---|---|
| `{"spec":{"kind":"sandbox_exec","command":"python hello.py"}}` → `Invalid params: spec.task must be a non-empty string`. The model adds a `task` it never wanted, or gives up. **445 of 948 calls (47%)** — the largest error class in `mcp_tool_calls`. | The call SUCCEEDS as the sandbox run it asked for. |
| `{"spec":{"kind":"sandbox_exec","task":"…"}}` → succeeds, runs a **sub-agent**, no error. The model believes its command ran. | Refused, naming `spec.command` and that `spec.task` belongs to `kind: subagent`. |
| `spec.cmd` / `spec.prompt` / `spec.script` → silently ignored, then blamed on `spec.task`. | Refused, naming the key and listing what `spec` accepts. |
| `flavor: "zee-workflow"` → a run row is created, the driver fetches, GitHub 404s, and the user learns MINUTES later via an inbox notification saying the rootfs download failed. | Refused immediately, listing `minimal`/`full`. No run, no wait, no notification. |

The refusal wording follows the one in-tree standard for model-facing errors
(`common/tool_args.rs`): name the argument, say what is expected, carry a literal
JSON example to copy. TEST-6 enforces that on every refusal this path emits, so a
future refusal cannot ship weaker than its siblings.

## 2. Infrastructure-integration walk

Every subsystem the change touches, and what each required:

| subsystem | interaction | outcome |
|---|---|---|
| **MCP tool-call history** (`mcp_tool_calls`) | a refusal is recorded by `McpSession::call_tool` like any other failed call | message text changes; no schema or recording change. This is also the table the live rig mined, so the fix is measurable there. |
| **Tool approval flow** | `spawn_background` is a WRITE → approval-gated; refusals happen AFTER approval, as before | unchanged. A user can still approve a call that is then refused — same as every other bad-argument refusal on this tool. |
| **Chat activity rail** | a refused tool call renders as a failed step via the background extension's existing contribution | unchanged; only the detail text differs. |
| **`workflow_runs` + the shared runner** | **the point of the change**: a refused spawn now creates NO row. Previously an invented flavor created a real run that failed minutes later. | strictly fewer rows; `resume.rs` / `startup_sweep.rs` see nothing new. |
| **Realtime sync** | a created run emits `SyncEntity::WorkflowRun`; a refusal emits nothing | strictly fewer spurious events. No new entity, no store change, no `sync_publish` call added. |
| **Notification inbox** | a completed/failed background run posts a row | a refused spawn no longer produces a doomed run and therefore no failure notification. |
| **SSE / streaming (chat path)** | a refused `flavor` returns a plain JSON-RPC error INSTEAD of opening the `execute_command` SSE stream | **checked**: the same handler already answers this route with plain `error_response(…)` JSON for the missing-`x-conversation-id` and `CONVERSATION_NOT_FOUND` cases, so a JSON error on a `tools/call` here is an existing, already-handled response shape — not a new one the MCP client has to learn. |
| **Per-conversation `conv_lock`** | the flavor check sits in the handler, before `execute_command_stream` (which takes the lock inside its task) | a refusal never acquires the conversation lock, so a bad flavor cannot stall a conversation. |
| **`code_sandbox` rootfs fetch/mount** | the whole point is to refuse upstream of `ensure_fetched` → `install_version` | no fetch is attempted for a value outside the catalog. |
| **Admin rootfs install** (`version_handlers.rs`) | deliberately NOT touched | its safe-token check stays, so an operator can still install a flavor published after this binary was built (DESIGN §Scope boundary, DEC-6). |
| **MCP system-server create / MCP user policy** | both had their own copies of the allow-list scan; both now delegate the predicate | error codes + statuses preserved verbatim and pinned (TEST-16, TEST-17). |
| **Workflow `sandbox.flavor`** | a THIRD unvalidated path to the same URL sink, found during this walk | deliberately out of scope (DEC-8) and reported, not silently fixed or silently ignored. |
| **Desktop** (`ziee-desktop` depends on `ziee`) | both modules compile into the desktop binary unchanged | no desktop override, no `CORE_MODULE_BLOCKLIST` change, no platform `cfg` involved. |
| **OpenAPI / generated TS** | MCP tool descriptors are runtime JSON-RPC, not aide/schemars types | no regen; `openapi.json` + `api-client/types.ts` byte-identical in both workspaces. |

## 3. Entity-lifecycle walk

The only entity this change touches is the background `workflow_runs` row.

- **ADD** — a row is now created only AFTER the arguments validate. Previously an
  invented flavor produced a real row whose driver was guaranteed to fail. Both
  origins are covered: the local spawn path is the only creator (there is no
  cross-device create for a background run), and TEST-13 asserts the row count is
  unchanged across a refused spawn.
- **MUTATE / terminal transition** — untouched; the runner drives the same
  lifecycle for every row that now exists.
- **REMOVE / DELETE** — untouched. A refused spawn creates nothing, so there is
  no orphan to clean up; the conversation-delete cascade sees strictly fewer rows.
- **`inputs_json` content** — the consumed `spec.kind` is now stripped before
  persistence. Rows written before this change may still contain it; **nothing
  reads a background run's `inputs_json` back** (verified: the two writes are
  `tools.rs:268`/`:503`, and `runner.rs:1616` is the workflow-run resume path,
  which the background surface refuses by `job_kind`). So no backfill and no
  migration are needed, and stripping is a narrowing, not a break.
- **ACCESS-LOSS** — ownership is verified before the row is created and again
  inside `execute_command_detached`'s `build_context`; neither check moved.
