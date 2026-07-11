# FIX_ROUND-3 — voice-model-mgmt

Fixed the 1 confirmed HIGH the round-3 re-audit surfaced, then ran a fresh blind
re-audit (round-4) — CONVERGED.

## Fixed (round-3)
- **[HIGH correctness] `.gguf` restart storm** (`auto_start.rs`) — added
  `desired_active_filename(name)` which resolves the auto-start comparison + persist key from
  the installed file's basename (`installed_model_path`, `.bin` OR `.gguf`), falling back to
  the default `.bin` name. Replaced all three `model_filename(&settings.model)` sites
  (`ensure_running` desired key, `apply_active_model_change`, `persist_running`). Now an
  activated `.gguf` model matches the warm-instance fast path instead of draining + respawning
  on every transcribe. Built-in `.bin` defaults unaffected.
- **[LOW resource-leak] Two-`file`-field upload temp leak** (`model_handlers.rs`) — a repeated
  `file` field now discards the previous streamed temp before replacing it.

## Re-audit (blind, round-4) — CONVERGED
A fresh blind agent traced the runtime paths and verified: (1) `desired_active_filename`
resolves the SAME basename `LocalDeployment::start` records (both funnel through the single
deterministic `installed_model_path`), so no key mismatch / restart storm for `.bin` or
`.gguf`; (2) the fresh/nothing-installed `.bin` fallback keeps built-in defaults working (no
regression); (3) no divergence between the launched file and the compared file; (4) the
two-file temp discard is correct (no double-free, no leak). A full-diff sweep surfaced no
remaining confirmed correctness/security/leak issue.

**New confirmed findings:** 0
