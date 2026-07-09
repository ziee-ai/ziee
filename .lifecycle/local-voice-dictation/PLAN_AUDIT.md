# PLAN_AUDIT — local-voice-dictation (full managed whisper runtime)

Audit of PLAN.md against the ziee codebase before any code. Grounded in five
research passes (build_helper, llm_local_runtime download/health, full
llm_local_runtime lifecycle map, web_search REST/perms/migrations, chat
composer) plus the external whisper.cpp facts (no official Linux/macOS binaries;
`whisper-server` OpenAI-compatible HTTP mode; ggml models on HF).

## Breakage risk

Purely additive — a new backend module `modules/voice/`, a new frontend admin
module + chat extension, three new tables in two new migrations, one new config
field, two new sync entities, one new external fork repo. No existing handler,
table, permission, route, provider, or store changes behavior. Shared-file edits
are append-only: one `pub mod voice;`, one `Option<VoiceConfig>` field
(`#[serde(default)]` ⇒ old configs parse unchanged), two new sync-entity enum
variants, `MODULE_ENTRIES` auto-registration (no central dispatch edit).

- **Risk — new external repo dependency (`ziee-ai/whisper.cpp`).** The runtime
  fetches binaries from it, exactly as `llm_local_runtime` already depends on
  `ziee-ai/llama.cpp`/`mistral.rs`. Mitigated: fail-soft (no release/asset ⇒
  the same `BinaryNotFound`/"build pending" path ⇒ mic self-disables), and
  air-gap operators pre-stage the binary in the cache dir. The fork + CI is a
  real one-time infra cost, but it is an established, twice-proven pattern.
- **Risk — a second long-lived subprocess + reaper.** The managed
  `whisper-server` is a new background process class. Mitigated by copying the
  proven hardening (`env_clear`, `PR_SET_PDEATHSIG`, `kill_on_drop`,
  loopback-verify) and the reaper's drain/idle-unload, and by keeping a SINGLE
  instance (whisper uses one model at a time, hot-swappable) rather than the
  per-model instance fan-out — strictly simpler than `llm_local_runtime`.
- **Risk — 16 MB global body limit** would reject audio uploads; the transcribe
  route opts into its own per-route `DefaultBodyLimit` (as `file`/`code_sandbox`
  routes do). No global change.
- **Risk — browser mic support.** Greenfield (`getUserMedia` grep-clean);
  unsupported/denied degrades to a disabled mic button, no composer regression.

## Pattern conformance

- ITEM-1..5 mirror `llm_local_runtime/engine/download.rs` + `runtime_version/*`
  + `binary_manager.rs` (repo slug, `archive_name`, `get_latest_version`,
  detached SSE download task, in-use delete guard, `check_for_updates`,
  `sync_cache`) and the `ziee-ai/llama.cpp` fork/CI/asset-naming. Copy-adapt
  rather than extending `EngineType` — a justified deviation (whisper is not an
  LLM; coupling it into `llm_local_runtime` would be wrong). Conformant.
- ITEM-6 mirrors `download_file` + `llm_model/storage.rs` cache discipline, with
  an explicit, documented departure from the git-LFS `llm_model` fetch path
  (whisper `ggml-*.bin` is a direct-URL single file, not GGUF). Conformant.
- ITEM-7..11 mirror `deployment/local.rs`, `engine/health.rs`, `auto_start.rs`,
  `reaper.rs`, and the `llm_runtime_versions`/`llm_runtime_settings`/instance
  tables (dropping `allow_unsigned_downloads`, removed upstream in migration 71).
  Conformant.
- ITEM-12/13 use a `use` + `admin::{read,manage}` split (web_search style)
  instead of the llm runtime's 9-perm split — a deliberate simplification
  toward the newer convention; admin is auto-covered by the `*` wildcard, only
  `voice::transcribe` needs a Users grant (mirrors migration 98/39). Conformant.
- ITEM-14/16 mirror the `web_search` config kill switch + module skeleton;
  ITEM-15 mirrors `file/handlers/upload.rs` (Multipart, magic-sniff, per-route
  cap) + the loopback-forward shape. Conformant.
- ITEM-17/18 mirror `runtime_version/handlers.rs` + `runtime_settings/handlers.rs`
  route/permission/SSE shape (static routes ordered before `{id}` params).
  Conformant.
- ITEM-21/22/23 mirror the extension-slot composer (`toolbar_actions`, TextStore,
  never `sendMessage`) and the `modules/llm-local-runtime/` admin UI (7 stores,
  SSE progress, `settingsAdminPages`, `sync:<entity>` self-gated refetch).
  Conformant.

## Migration collisions

`ls migrations/` — highest existing is
`00000000000131_rewrite_hub_ids_phibya_to_ziee_ai.sql`. The plan uses **133**
(`create_voice`: `voice_runtime_versions` + `voice_runtime_instance` +
`voice_runtime_settings`) and **134** (`grant_voice_permissions_to_users`) — both
free. Naming + create/grant split follow 97/98, 100/101, 103/104. Singleton PK
idiom (`id BOOLEAN PK CHECK(id=TRUE)`) and the `array_append` grant idiom already
in-tree. No FK collisions (new tables reference only themselves + optional
version pin).

## OpenAPI regen

Required — ITEM-17/18 add a substantial typed surface
(`Voice.transcribe`, `Voice.listVersions`/`checkUpdates`/`downloadVersion`/
`deleteVersion`/`setDefaultVersion`/`syncCache`/`downloadEvents`,
`Voice.getSettings`/`updateSettings`, `Voice.modelStatus`/`modelDownload`,
`Voice.getInstance`/`restart`/`stop`/`logsStream`) + `JsonSchema` DTOs + two sync
entities. `just openapi-regen` regenerates BOTH `ui/` and `desktop/ui/`
`openapi.json` + `api-client/types.ts`; the golden `types_ts_parity` test fails
on drift, so regen is mandatory and verified ([[project_openapi_regen_both_binaries]]).
Generated artifacts are excluded from the phase-6 coverage law and don't classify
the diff as UI work (the hand-written `ui/**` items do — hence the e2e tests).

---

## Per-item verdicts

- **ITEM-1** — verdict: CONCERN — new external `ziee-ai/whisper.cpp` fork + release CI is required infra (out-of-monorepo); low risk (twice-proven with llama/mistral) but a real prerequisite. Confirm whisper.cpp's CMake target name for the server (`whisper-server`) at the pinned tag and that a static build is feasible per platform.
- **ITEM-2** — verdict: PASS — direct adaptation of `engine/download.rs`; keep the mirror-env seams debug-only.
- **ITEM-3** — verdict: PASS — clone of `runtime_version/{models,repository}`; single-engine so the `engine` column can be dropped.
- **ITEM-4** — verdict: CONCERN — `download_task.rs` (DashMap + broadcast + shutdown race + reload-safe re-subscribe) is subtle; copy carefully incl. terminal-entry replace-on-retry.
- **ITEM-5** — verdict: PASS — mirrors `binary_manager.rs`; reuse `gpu_detect::host_platform/arch`.
- **ITEM-6** — verdict: CONCERN — no existing generic direct-URL model downloader; write fresh from `download_file`. sha256 pinning needs a stable known-hash table (HF has no per-file sidecar).
- **ITEM-7** — verdict: CONCERN — depends on the exact `whisper-server` CLI flags (`--host/--port/-m/-l`) + its `/inference` request/response shape at the pinned tag; verify against `--help` before wiring.
- **ITEM-8** — verdict: PASS — `engine/health.rs` is a pure, clock-injectable copy target.
- **ITEM-9** — verdict: CONCERN — single-instance model swap/hot-reload differs from the per-model `auto_start`; simplify the OnceCell/HEALTH maps to a single key.
- **ITEM-10** — verdict: PASS — `reaper.rs` copy; single-instance drain.
- **ITEM-11** — verdict: PASS — migration mirrors 20/66/67/68 minus per-model + `allow_unsigned_downloads`; `cargo clean` once so build.rs re-runs migrations.
- **ITEM-12** — verdict: PASS — `PermissionCheck` structs per `web_search/permissions.rs`.
- **ITEM-13** — verdict: PASS — 133/134 free (132 taken by add_openrouter on origin/main); grant mirrors 98.
- **ITEM-14** — verdict: PASS — `Option<VoiceConfig>{enabled}` additive/back-compatible.
- **ITEM-15** — verdict: CONCERN — needs `just openapi-regen`; careful temp-file cleanup + WAV validation + forwarding to the loopback whisper-server; ensure-running failure must return a clear 409/503, not a 500.
- **ITEM-16** — verdict: PASS — `web_search`/`llm_local_runtime` module skeleton; pick a free `MODULE_ENTRIES` order.
- **ITEM-17** — verdict: CONCERN — `just openapi-regen` + the fail-closed in-use delete guard + SSE route ordering; mirror `runtime_version/handlers.rs` exactly.
- **ITEM-18** — verdict: CONCERN — `just openapi-regen` + `sync:voice_settings`/instance redaction of `base_url` for non-admins.
- **ITEM-19** — verdict: CONCERN — mechanical but mandatory in BOTH workspaces; keep `types_ts_parity` green.
- **ITEM-20** — verdict: CONCERN — macOS Tauri needs `NSMicrophoneUsageDescription` or `getUserMedia` fails silently; Windows WebView2 verified on the Windows build host. Platform-gated, non-blocking for web.
- **ITEM-21** — verdict: PASS — extension-slot composer supports this; in-browser 16 kHz WAV keeps the server ffmpeg-free.
- **ITEM-22** — verdict: CONCERN — large UI surface (mirrors a ~2.4 kLOC module); the SSE download-progress store must survive page reload (list-active + re-subscribe) and self-gate on `VoiceAdminRead`.
- **ITEM-23** — verdict: CONCERN — new render states must have gallery cells or `check:state-matrix` (inside `npm run check`) + the UI gate fail; budget the cells.
