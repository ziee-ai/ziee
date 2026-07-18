# Chunk `notification` — TESTS-MOVED

## Moved INTO `ziee-notification` (with the code)

`models.rs` and `permissions.rs` carried NO `#[cfg(test)]` units, so no in-source
unit tests moved. `cargo test -p ziee-notification` → **0 tests** (compiles clean).

The moved types' behavior (the `NewNotification` builder chain, the `Notification`
`FromRow` mapping, the serde wire shapes) is exercised by ziee's retained
integration suite (`tests/notification/…` / the scheduler-dispatch path), which
drives the real `repository.rs` (`query_as!`) + `events.rs` (`create_and_emit`)
against a live DB. The compile-time proof that the moved types still match the
moved schema is `cargo check -p ziee` (the 10 `query_as!` in the retained
`repository.rs` verify against the merged build DB that now includes the moved
migrations).

## Stayed in ziee (app-coupled — not moved)

| Test home | Why it stayed |
|---|---|
| `tests/notification/*` (integration) | Drive the retained `repository.rs`/`events.rs`/`handlers.rs`/`prune.rs` through the app `TestServer` (names `RequirePermissions`/`Repos`/`SyncEntity`/`module_api`) against a live DB. |

No behavioral assertion was edited (only the one trait-import line in
`permissions.rs`) — the MOVE-preserves-behavior discipline holds.
