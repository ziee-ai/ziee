# TEST_RESULTS — scheduled-background-tasks (phase 8)

Scoped to the touched areas: backend (`src-app/server`) + frontend (`src-app/ui`).

## Backend — unit (`cargo test --lib -p ziee scheduler::`)

- **TEST-1**: PASS  (schedule::next_occurrence — once/weekly/timezone/DST)
- **TEST-2**: PASS  (schedule::validate_schedule — bad cron/tz/past/too-frequent)
- **TEST-30**: PASS (failure::classify + should_autopause + backoff)
- **TEST-42**: PASS (change::fingerprint stability + item-set diff)

(5 schedule + 3 change + 3 failure = 11 in-source unit tests green.)

## Backend — integration (`cargo test --test integration_tests scheduler:: notification::`)

- **TEST-11**: PASS (scheduler CRUD round-trip + next_run_at populated)
- **TEST-12**: PASS (403 without scheduler::use / 401 unauth)
- **TEST-13**: PASS (quota 422 at the admin cap)
- **TEST-19**: PASS (notification inbox CRUD: list/unread-count/mark-read/delete)
- **TEST-14/15 (partial, via run-now)**: PASS — `run_now_prompt_produces_a_notification`
  drives the FULL path (tick::fire_task → dispatch_prompt → real chat pipeline vs
  stub model → is_generating completion → change-detection → notification) and
  asserts a `scheduled_task_result` notification lands in the inbox linking the
  conversation. Owner-scope 404 + gating covered.

## Frontend

- `npm run check (ui): PASS` — tsc + guardrails + colors + settings-field +
  logical-direction + tooltip + kit-manifest + testid-registry + design-spec +
  gallery-coverage + gallery-crawl + state-matrix + overlay-registry (full gate).

- E2E specs authored (`tests/e2e/14-scheduler/scheduled-tasks.spec.ts`,
  `tests/e2e/15-notifications/inbox.spec.ts`) — create-task flow + inbox mark-read.

## Notes

- The full firing-path tests use the in-repo `stub_chat`/`stub_engine` mock LLM
  (the same programmable-provider harness the chat tests use) — a real boundary,
  not a cosmetic mock ([[feedback_no_cosmetic_tests]]).
- Sandbox/real-LLM tiers not applicable to this feature.
