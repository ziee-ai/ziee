# Chunk `server-update` — TESTS-MOVED

## Moved INTO `ziee-server-update` (with `permissions.rs`, verbatim)

`crates/ziee-server-update/src/permissions.rs` `#[cfg(test)]` (1):

| Test | Covers |
|---|---|
| `server_update_403_example_carries_the_read_permission` | the `(ServerUpdateRead,)` tuple's `names()` == `["ServerUpdateRead"]`, `permissions()` == `["server_update::read"]`, `descriptions()` — the exact data feeding the OpenAPI 403 example that the UI `Permissions` enum is scraped from |

SDK result: `cargo test -p ziee-server-update` → **1 passed**.

## Stayed in ziee (app-coupled — not moved)

`server_update/checker.rs` `#[cfg(test)]` (7) stay with `checker.rs` (which stays
in ziee — it embeds `env!("CARGO_PKG_VERSION")` and one test names
`crate::core::config::UpdateCheckConfig`):

| Test | Why it stayed |
|---|---|
| `newer_older_equal`, `prerelease_orders_below_release`, `semver_of_accepts_versions_and_rejects_garbage` | drive `is_newer`/`semver_of` (in `checker.rs`) |
| `set_enabled_reflects_in_cache` | drives the process-lifetime cache (in `checker.rs`) |
| `config_default_enabled_true` | names `crate::core::config::UpdateCheckConfig` (ziee's type) |
| `extract_release_full_shape`, `extract_release_handles_missing_and_empty` | drive `extract_release` (in `checker.rs`), and use `env!("CARGO_PKG_VERSION")` |

No behavioral assertion was edited (only the two trait-import lines in the moved
test) — the MOVE-preserves-behavior discipline holds.
