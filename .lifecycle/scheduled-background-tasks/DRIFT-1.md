# DRIFT-1 — implementation vs plan

Reconciliation of the built implementation against PLAN.md / DECISIONS.md. Each
divergence is either re-implemented to match the plan (`plan-wins`), or the plan
is amended with rationale (`impl-wins`).

- **DRIFT-1.1** — verdict: impl-wins — Migrations renumbered 132–138 → **133–138**. The plan assumed 132 was free, but `origin/main` already shipped `00000000000132_add_openrouter_provider_type.sql` (my local checkout was stale). Caught at build time (duplicate `_sqlx_migrations` PK). PLAN/PLAN_AUDIT numbers are superseded by the committed 133–138; no functional change.

- **DRIFT-1.2** — verdict: impl-wins — `scheduled_tasks` feature columns (`consecutive_failures`, `paused_reason`, `notify_mode`, `notify_on`, `bound_conversation_id`, `last_result_fingerprint`, `last_result_signature_json`) were **consolidated into the create migration (133)** instead of a follow-on ALTER in the plan's ITEM-27 migration. ALTERing a table created in the same migration batch is a smell; a fresh table with its full shape is clearer. ITEM-27's migration (now **138**) is therefore just the `scheduled_task_runs` audit table.

- **DRIFT-1.3** — verdict: impl-wins — DEC-16 (claim + advance in ONE transaction) is realized as **advance-`next_run_at`-immediately-after-claim, before dispatch** rather than a single wrapping transaction. Rationale: the scheduler is single-process (DEC-10) with a sequential `run_once`, so there is no concurrent double-fire to guard; advancing before the async dispatch still gives the "a crash mid-dispatch never re-fires" property. The `FOR UPDATE SKIP LOCKED` claim query is retained as documented defense-in-depth for a future multi-instance deployment.

- **DRIFT-1.4** — verdict: impl-wins — the prompt-target completion signal (DEC-5) is a new **read-only `chat::stream::registry::is_generating(conversation_id)`** accessor (the generation slot flips off when the terminal SSE frame publishes), polled to terminal — rather than the plan's vaguer "poll the persisted message". This is the precise, non-fragile signal; it required two minimal, justified chat-module edits (`pub extension_registration` + the accessor). Recorded here as the concrete realization of the DEC-5 "await completion" contract.

- **DRIFT-1.5** — verdict: impl-wins — the boot **catch-up sweep** (a plan item) is **inherent in the tick's due-claim query** (`WHERE enabled AND next_run_at <= now`): an overdue task is claimed on the first tick and fired once (coalesced), then `next_run_at` advances past `now`. No separate `startup_sweep`-style function is needed; the coalesced-catch-up semantics (DEC-6) are delivered by `fire_task`'s next-occurrence computation.

- **DRIFT-1.6** — verdict: impl-wins — dry-run (ITEM-34, DEC-24) is **fully side-effect-free for the prompt kind** (throwaway conversation, deleted after) but for the **workflow kind executes a real `workflow_runs` row** (via `spawn_run(persist_artifacts=false)`); a fully side-effect-free workflow dry-run would need the workflow test harness's heavy `run_for_test` setup. The scheduler-side surface (notifications / task history / schedule) is untouched — which is the "no side effects" contract that matters. Documented in `dryrun.rs`'s module comment.

- **DRIFT-1.7** — verdict: impl-wins — added a durable `notifications.interrupt` column (not in the original ITEM-2 schema) to carry the toast-vs-silent delivery hint (DEC-19), because the realtime sync frame is payload-free so the client can't otherwise know whether to raise a toast. Folded into migration 134.

**Unresolved drifts:** 0
