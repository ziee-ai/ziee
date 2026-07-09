// Integration tests for the scheduler module (Tier 2).
//
// CRUD + owner-scoping + permission gating + the per-user quota (422).
// Firing (tick/dispatch) + change-detection + failure auto-pause are exercised
// by the in-source unit tests (schedule/change/failure engines) + would need a
// mocked LLM/workflow capture harness for the full path.

mod crud_test;
