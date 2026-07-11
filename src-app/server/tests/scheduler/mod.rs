// Integration tests for the scheduler module (Tier 2/3).
//
// crud_test: CRUD + owner-scoping + permission gating + the per-user quota (422).
// tick_test: the TICK-DRIVEN scheduled firing path (via the SCHEDULER_TICK_MS
//   debug seam + the stub-model chat harness) — a scheduled `once` prompt fires,
//   advances/disables, records its outcome + run history + notification; and
//   run-now's no-schedule-mutation contract. (See notification/inbox_test.rs for
//   the run-now → dispatch → notification path.)
// The change-detection `on_change` diff + the failure→auto-pause transition are
// covered by the in-source unit tests (change.rs / failure.rs); a fast
// integration test of on_change is blocked by the 300s min-interval floor
// between two scheduled firings, and a clean terminal-failure injection would
// need a stub error seam the scheduler doesn't expose.

mod continue_in_chat_test;
mod crud_test;
mod dispatch_behavior_test;
mod runs_timeline_test;
mod sync_emit_test;
mod test_fire_test;
mod tick_test;
mod validation_test;
