# BASE — voice-model-mgmt conflict surface

Branch cut from `origin/main` @ `304f4a011` (verified current tip at plan time).

## Migrations
- Highest existing migration on main: **`00000000000154_add_voice_streaming_settings.sql`**.
- This branch adds **`00000000000155_create_voice_models.sql`** (next free number): creates the
  `voice_models` table, `ALTER`s `voice_runtime_settings` to add `model_source_repo` (additive
  column with a DEFAULT — no backfill), AND `ALTER`s `voice_runtime_instance` to add a `state`
  value `CHECK` constraint (F7; safe — the singleton's existing state is always a valid name).
  No renumber needed.
- Collision risk: any concurrently-merging branch that also claims `155` — none known. The
  merge-gate (C2) re-checks migration-number collision against real main at merge time; if
  main advances past 154 before merge, bump this to the next free number.
- No new **permission** migration (feature reuses `voice::admin::{read,manage}`; those grants
  already exist via migration 152 + the Administrators `*` wildcard).

## Files main is actively changing that this branch also touches
- The voice module (`src/modules/voice/**`) is the subject of recent main commits
  (`0dce131d4` merge of `feat/streaming-voice-transcription`, `08c039f0a`, `6c0a1abf7`,
  `0a40c80d3`, `304f4a011`). This branch edits `voice/{model.rs,models.rs,repository.rs,
  handlers.rs,routes.rs,auto_start.rs,mod.rs,instance_handlers.rs}` and adds new files.
  Because main just merged the streaming-voice work, the base already contains it — no
  in-flight divergence expected, but re-verify base == current `origin/main` before merge.
- `src/modules/sync/event.rs` — adding a `VoiceModel` enum variant (append-only near
  `VoiceRuntimeVersion`); low collision risk (additive).
- Frontend `src/modules/voice/**` + `src/dev/gallery/coverage.ts` — additive.

## OpenAPI regen implied?
- **Yes.** New `Voice.*` endpoints + request/response types → `just openapi-regen` regenerates
  BOTH `src-app/ui/` and `src-app/desktop/ui/` (`openapi.json` + `api-client/types.ts`).
  These generated files are excluded from the Phase-6 coverage law and the Phase-3/8 frontend
  gates (they're deterministic codegen). The golden parity test
  (`openapi::emit_ts::tests::types_ts_parity`) must stay green.
- Regen produces a large positional (key-order) diff in `openapi.json` with a small content
  delta — verify the content delta with `comm` on sorted files and record a drift entry if noisy.

## Desktop (R2-3)
- `src-app/desktop/ui/` carries hand-written overrides. This feature is server-backed voice
  admin UI; confirm no security-relevant voice logic diverges in the desktop copy after regen.
  Desktop embeds the server, so the voice admin surface may be blocked by
  `CORE_MODULE_BLOCKLIST` / a config flag on desktop — verify the voice module's desktop
  disposition and don't assume the new routes are reachable there.
