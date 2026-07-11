# FIX_ROUND-2 — voice-model-mgmt

Fixed the 4 confirmed findings the round-2 re-audit surfaced, then ran a fresh
blind re-audit (round-3).

## Fixed (round-2)
- **[HIGH correctness] Library models couldn't run** (`model.rs`) — `ensure_model` now
  returns any INSTALLED file first (via `installed_model_path`, `.bin` OR `.gguf`),
  regardless of the 4-name built-in list; only an ABSENT model requires a known catalog
  name (for the pinned auto-download). An activated `large-v3` now actually serves.
- **[MED correctness] `.gguf` orphan** (`model.rs`/`instance_handlers.rs`) — `installed_model_path`
  + `model_present` + `get_model_status` resolve BOTH `.bin` and `.gguf`.
- **[MED resource-leak] Upload temp leak on later-field error** (`model_handlers.rs`) — a
  `TempGuard` RAII removes the streamed temp on EVERY early return, disarmed only after finalize.
- **[LOW concurrency] Prune held DashMap guard across await** (`model_download_task.rs`) —
  `prune_terminal_tasks` now snapshots (key, Arc) first, then locks outside iteration.

## Re-audit (blind, round-3) — 1 NEW confirmed
The round-3 re-audit verified all four above sound, but found the `.gguf` fix INCOMPLETE:
- **[HIGH correctness]** the resolution layer found `.gguf` so `LocalDeployment::start` records
  `active_model = ggml-<name>.gguf`, but the auto-start COMPARISON layer
  (`live_handle_if_current`/`detect_crash`/`apply_active_model_change`/persist) still derived
  the desired key via `model_filename` (`.bin`), so a `.gguf` model never matched the warm
  instance → drain + respawn on EVERY transcribe (restart storm).

Fixed in round-3 (this commit): a `desired_active_filename` helper resolves the comparison +
persist key from the installed file's basename (falling back to `.bin`); all three auto_start
sites use it. Also closed a LOW two-`file`-field upload temp leak.

**New confirmed findings:** 1
