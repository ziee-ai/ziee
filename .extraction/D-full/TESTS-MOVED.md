# Chunk D-full — TESTS-MOVED

D-full is an equivalence-preserving MOVE of runtime window/boot-orchestration
glue — no new behaviour, so no NEW tests.

## Tests moved: NONE

- `create_main_window` had **no** unit tests (pure per-OS Tauri window
  construction — runtime-only, not unit-testable headlessly), so nothing moved
  with it.
- The boot→window spawn skeleton had **no** unit tests either.

## Tests that STAY app-side (unchanged, re-verified)

The 7 in-source `#[cfg(test)]` tests in ziee-desktop `modules/backend/mod.rs`
test **config + storage-key**, not the window, so they stay with the app and are
untouched by D-full:

- `ensure_persistent_storage_key_creates_64_hex_chars_on_first_call`
- `ensure_persistent_storage_key_returns_same_key_on_second_call`
- `ensure_persistent_storage_key_persists_to_disk`
- `ensure_persistent_storage_key_regenerates_when_existing_is_too_short`
- `create_desktop_config_sets_secrets_storage_key`
- `create_desktop_config_disables_server_update_check`
- `create_desktop_config_returns_same_storage_key_across_calls`

**Verified: `cargo test -p ziee-desktop --lib modules::backend::` — 7 passed, 0
failed** (I edited this file to rewire `start_backend_server`, so I ran its module
tests to prove no collateral breakage).

## Runtime proof is a post-merge E2E boundary (honest caveat)

The moved window/boot lifecycle is **runtime-only** — it opens a real Tauri
window driven through `ServerBoot::boot()`, which the Bash-tool integration
harness cannot exercise (no display server, no live embedded-server GUI boot).
Its behavioural proof is the desktop launch + permanent-session + window-shows-on-
boot-success-AND-failure E2E, the same boundary BG-3's BOUNDARY.md and the Chunk
D STOP_REPORT already named for the auth-carrying live desktop boot. That is the
orchestrator's post-merge desktop-E2E step, **not** a STOP: the seam is threaded,
the code compiles on all three OS `#[cfg]` arms (linux built here), and the golden
is byte-identical. No behavioural assertion was weakened to make a suite green (no
`#[ignore]` / `.skip` added).
