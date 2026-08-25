# DECISIONS — background-spawn-loop-guard

### DEC-1: The spawn cap — fixed constant, new admin-configurable settings row, or reuse an existing tunable?
**Resolution:** REUSE the existing `agent_admin_settings.fan_out_max_threads`
(default 6, range 1..=64) as the per-conversation concurrent+queued background-run
cap. No new migration, no new permission, no new sync entity, no new admin card.
**Basis:** codebase + convention. The lifecycle's configurable-settings rule
mandates that an operational tunable be admin-configurable, and the task brief
directs checking `agent_admin_settings` FIRST for an existing fan-out/spawn
guardrail to reuse. `fan_out_max_threads` is documented in its migration as
"bounds fan-out CONCURRENCY" — the semantic twin of "max concurrent background
runs" — is already admin-editable via `PUT /api/agent/settings` (gated
`agent::settings::manage`) with bounds validation and a sync entity, and is
already read on the background sub-agent path (`drive_subagent_turn` calls
`Repos.agent.get_admin_settings()`). Reuse keeps the change LIGHT tier (a new
singleton settings row would flip it to HEAVY: migration + `::settings` perm +
sync + admin card + deny test) and avoids a redundant second knob for the same
"how many concurrent sub-agents" concept.

### DEC-2: The dedup window — configurable or a fixed internal constant?
**Resolution:** A FIXED named constant `SPAWN_DEDUP_WINDOW_SECS = 300` (5 minutes)
in `background_mcp::tools`, with a documented rationale. It is NOT an admin
setting.
**Basis:** convention. This window is an internal IDEMPOTENCY / coordination
constant, not an operational tunable an operator would size to a workload — the
same category as `resume.rs`'s `RESUME_MAX_IDLE_WAIT` (explicitly documented there
as "an INTERNAL coordination timeout, not an operator tunable"). Its only job is
to catch the completion→re-inject→re-spawn loop, which fires within seconds; 5
minutes comfortably covers that while still allowing a deliberate re-run of the
same task later. Structured as a named const (not a magic number) so it can be
promoted to configurable later without a rewrite.

### DEC-3: Which run statuses does the dedup window match (so a legitimate re-run is not blocked)?
**Resolution:** Dedup matches a same-(conversation, job_kind, inputs_json) run
that is EITHER non-terminal (`pending`/`running`/`waiting`/`resumable`, any age)
OR recently terminal-successful/failed (`completed`/`failed` AND
`created_at > now() - window`). It deliberately EXCLUDES `cancelled` and terminal
runs older than the window.
**Basis:** convention + the diagnosis. Non-terminal-of-any-age catches a still-in-
flight duplicate (the concurrent-loop case); recent `completed`/`failed` catches
the completion-driven re-spawn loop. Excluding `cancelled` respects an explicit
user "stop" (a re-spawn after a cancel is intentional, not a loop); excluding
old terminal runs lets a user legitimately re-run the same task later.

### DEC-4: What does the per-conversation cap count?
**Resolution:** All NON-TERMINAL background runs for the conversation
(`job_kind <> 'workflow'`, `status IN ('pending','running','waiting','resumable')`)
— across BOTH kinds (`subagent` + `sandbox_exec`). The just-inserting run is not
pre-counted (cap check precedes the INSERT), so cap=N permits N concurrent and
refuses the N+1th.
**Basis:** INV-2 ("concurrent+queued background runs per conversation") +
CODING_GUIDELINES §4/§5 (bound the unbounded-growth path). Terminal runs are done
and do not consume capacity; workflow runs are a separate surface.

### DEC-5: Is a Duplicate a tool ERROR or a normal tool RESULT? And OverCap?
**Resolution:** A Duplicate returns a normal (non-error) tool RESULT carrying the
existing `run_id` and an "already running/queued — do not spawn a duplicate" note.
An OverCap returns a tool ERROR (`AppError::bad_request("BACKGROUND_SPAWN_CAP_EXCEEDED", …)`).
Both create NO new run.
**Basis:** the invariants verbatim — INV-1 says the duplicate "returns a clear
'already running/queued' RESULT instead of a second run"; INV-2 says over-cap
"returns a clear over-cap ERROR and creates NO run". A Duplicate-as-result is also
the better loop-breaker: it hands the model the existing run_id and tells it to
wait, rather than an error the model may retry.

### DEC-6: Where is the check+insert made race-safe (TOCTOU)?
**Resolution:** In ONE transaction in `workflow/repository.rs::insert_background_run_guarded`,
holding `pg_advisory_xact_lock(hashtextextended(conversation_id::text, 0))` across
the dedup SELECT, the cap COUNT, and the INSERT. No new schema/unique index.
**Basis:** CODING_GUIDELINES §4 ("No TOCTOU … guard+write in one txn"). A
per-conversation advisory xact-lock serializes concurrent spawns for the SAME
conversation (the only race that matters — two devices/tabs, or a re-inject racing
a manual spawn) while leaving different conversations fully parallel. Chosen over a
partial UNIQUE index because an index requires a migration on the shared
`workflow_runs` table (→ HEAVY tier + collision surface with the concurrent
read-path worktree) and cannot express the time-window/status predicate.
