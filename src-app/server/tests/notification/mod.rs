// Integration tests for the notification inbox (Tier 2/3).
//
// `inbox_test` drives the FULL firing path end-to-end: run-now a prompt-target
// scheduled task (real chat pipeline against a stub model) and assert the
// resulting notification lands in the inbox, then exercises the inbox CRUD
// (list / unread-count / mark-read / read-all / delete / owner-scope / gating).

mod inbox_test;
