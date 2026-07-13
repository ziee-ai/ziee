# UX Design — Follow-up & Series (FB-7 deliverable)

The comprehensive jobs-to-be-done + per-surface design mandated by FB-7, for the
four capabilities the human selected this round: **Ongoing thread**, **Inline
results + what-changed timeline**, **Series-level follow-up**, **Real-content
follow-up fidelity**. Every surface the feature exposes is reconciled against the
JTBD below before any ITEM is written.

## Jobs-to-be-done (the mental model)

A scheduled task is a **standing assignment to an assistant** whose value is the
**evolving stream of results and what changed across it** — not any single firing.
So a real user wants to:

- **J1 — Scan the series** and see, per run, *what it produced* and *what's new*,
  without leaving the page or forking anything.
- **J2 — Read one result** in full quickly.
- **J3 — Reply into the ongoing thread** that already holds the whole series
  (for a prompt task that is the existing `bound_conversation_id`).
- **J4 — Follow up on the REAL result** (actual assistant answer / workflow
  output + artifacts), not a truncated paraphrase.
- **J5 — Ask across runs** ("compare this week to last", "trend over the month").

## Surface inventory (every component/interface — reconciled)

### S1 — Scheduled Tasks list page (`ScheduledTasksPage.tsx`)
Precedent/twin: the settings-list container it already uses. Unchanged shell.
Per-task row gains a task-level follow-up cluster (see S3). Mobile: row already
stacks; the new actions collapse into the existing overflow.

### S2 — Task row header
Add, next to Run-now/Edit, a **primary "Open thread"** action **for prompt tasks
that have fired** (`target_kind=='prompt' && bound_conversation_id`) → navigates
to `/conversations/{bound_conversation_id}` (J3). Absent for workflow tasks (no
thread) and for a never-fired prompt task (disabled with tooltip "runs once, then
you can open the thread"). This is the single highest-leverage move: the series
already lives in that one conversation; we just stop hiding it.

### S3 — Runs panel (per task, "Show runs")
Precedent for bounding: the numbered `ListPagination` idiom (settings/detail
list). Today it fetches a flat 100 and renders all — **bound it**: load the
latest page (default 10) with `ListPagination`, "Showing N of M". A task-level
**"Discuss recent runs ▾"** control (J5) sits in the panel header (chooser: last
5 / 10 / all-loaded).

### S4 — Run row (the atomic result unit) — the timeline
Today: `timestamp — status (error_class)` + skipped note + "Continue in chat".
Redesign (J1/J2):
- **What-changed marker** — a badge from persisted change-detection:
  `NEW ×N` (new_items) / `changed` / `no change` / `failed`. Color via semantic
  tokens (success/warning/destructive/muted) — never raw hex.
- **Result preview** — one clamped line of the run's result text (~280 chars).
- **Expand (J2)** — click the row (or a chevron) to reveal the fuller result
  text inline (up to the notification cap ~800) + skipped-tools + failure message
  for a failed run. No fork needed to read.
- **Actions** (right-aligned / overflow on mobile):
  - prompt run → **"Open thread"** (resume the bound conversation, primary) +
    **"New side chat"** (the existing fork, secondary/overflow).
  - workflow run → **"Continue in chat"** (fork, now carrying real output — S7/J4).

### S5 — Form drawer (`ScheduledTaskFormDrawer`)
No new fields this round. If a prompt task has a thread, the drawer footer may
show a passive "Open thread" link for parity, but the primary entry stays on the
row (S2/S4). Untouched otherwise.

### S6 — Notifications (inbox)
Already carries title + body (result digest, capped) + `conversation_id` /
`workflow_run_id` links + skipped note. Keep as-is; it is the push channel.
Ensure the notification's link opens the SAME thread (prompt) the panel "Open
thread" does, so push and pull are consistent.

### S7 — The thread / continue-seeded conversation (J3/J4)
- **Prompt series (J3)** — the bound conversation IS the home; every firing has
  appended a turn, so it reads as a dated series the user scrolls and replies
  into. "Open thread" lands here. (Deep-link-to-turn is a nice-to-have, deferred.)
- **Single-run follow-up fidelity (J4)** — `continue_run_in_chat` must seed the
  REAL result: for a prompt run, the actual last assistant text (not a 2000-char
  paraphrase); for a **workflow** run — which today seeds *nothing* — pull the
  run's `final_output_json` digest (`summarize_workflow_output`) **and** its
  persisted artifacts (files linked via `files.workflow_run_id`), attached as
  provider-routed file ContentBlocks.
- **Series follow-up (J5)** — a new "discuss last N runs" path seeds a
  conversation with the last N runs' previews + their change deltas, so the user
  can ask trend/compare questions.

### S8 — States (all surfaces)
- **Empty** — "No runs yet" (exists); a never-fired prompt task shows
  "Open thread" disabled with the run-once tooltip.
- **Failed run** — destructive marker + the error message on expand; follow-up
  still available ("discuss why it failed").
- **Loading** — existing `Spin`; the paged panel shows a skeleton on page change.
- **Mobile (~390px)** — run rows stack; preview clamps to one line; per-run
  actions collapse to an overflow menu; the timeline stays vertical. Gallery must
  include the narrow-viewport state.

## Backend additions implied (accurate to current code)

- `scheduled_task_runs` stores **no result text and no change delta** today
  (only status). Add **migration 155**: `result_preview TEXT NULL` (capped) +
  `change_summary_json JSONB NULL` (`{changed, new_count, new_items?}`), written
  in `finalize_success`. This powers S4's preview + what-changed badge and the
  series seed's deltas. FULL content for a single-run follow-up (J4) is pulled
  LIVE at continue time (conversation history / workflow output+artifacts) — not
  duplicated onto the run.
- `bound_conversation_id` is **already** on the task API (models.rs:82) → S2/S4
  "Open thread" needs no backend change.
- New endpoint for J5: `POST /scheduled-tasks/{id}/continue-series` (body/query
  `limit`) reusing the `continue_run_in_chat` scaffolding, aggregating the last N
  runs.
- Runs pagination: `list_task_runs` gains `page`/`per_page` (mirror the existing
  paginated list handlers); store + `ListPagination` on the client.

## Product decisions (human-chosen via option pickers — LOCKED)

- **DEC-A (config rule) — CHOSEN: fixed UX constants.** result-preview length
  (~280) + series default N (5) + runs page size (10) are named constants, not
  admin settings (presentation defaults; retention stays the admin knob).
- **DEC-B — CHOSEN: "Open thread" + keep fork.** prompt-task primary action =
  "Open thread" (resume bound conversation); the existing fork survives as
  secondary **"New side chat"**.
- **DEC-C — CHOSEN: chooser {5, 10, all-loaded}, default 5** for "Discuss recent
  runs".
- **DEC-D — CHOSEN: synthesized assistant turn.** the follow-up seed creates an
  **assistant** message carrying the real result (prompt run → the actual last
  assistant text; workflow run → the `final_output_json` digest **+ artifacts as
  file ContentBlocks on that assistant message**; series → one assistant summary
  of the last N runs' results + deltas), and the user then replies. Reads as if
  the assistant reported the result. NOT a user-message-embed. (Note: for a
  prompt run this is the *real* assistant text, not a fabrication; for
  workflow/series it is a synthesized-but-truthful assistant framing of the
  actual output.)

## Reuse targets (no bespoke layouts)
`ListPagination` (paging) · the existing run-row/panel container · the file
ContentBlock attach path used by chat/project file processors (workflow
artifacts) · `summarize_workflow_output` (workflow digest) · `continue_run_in_chat`
(seed scaffolding) · semantic color tokens for the change badges.
