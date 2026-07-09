# PLAN_AUDIT — local-voice-dictation

Audit of PLAN.md against the ziee codebase, done before any code is written.
Findings grounded in the four research passes (build_helper, llm_local_runtime,
web_search REST/permissions/migrations, chat composer).

## Breakage risk

The feature is **purely additive** — a new backend module (`modules/voice/`), a
new frontend admin module + chat extension, two new migrations, one new config
field, one new sync entity. No existing handler, table, route, permission, or
store is modified in a behavior-changing way. The only edits to shared files are
append-style:

- `build.rs::setup_external_binaries()` — one new warn-and-continue helper call;
  the existing binary helpers are untouched (same pattern as adding biomcp).
- `core/config.rs` — one new `Option<VoiceConfig>` field with `#[serde(default)]`,
  so existing config files parse unchanged (absent section ⇒ enabled default).
- `modules/mod.rs` — one new `pub mod voice;` line; module auto-registers via the
  `MODULE_ENTRIES` distributed slice, so no central dispatch edit.
- `sync` entity enum — one new `VoiceSettings` variant; the generated
  `sync:${entity}` TS key derives automatically on the next regen.
- **Risk:** the whisper.cpp cmake build adds a build-host dependency (cmake +
  C/C++ toolchain). Mitigated because (a) pgvector already requires
  `build-essential` (make+gcc) on the build host, so a compiler is assumed, and
  (b) the helper is fail-soft — no cmake ⇒ zero-byte stub ⇒ the module
  self-disables and the whole server still builds and runs. This is the explicit
  hard requirement and matches pgvector/biomcp exactly.
- **Risk:** the global 16 MB `DefaultBodyLimit` would reject larger uploads; the
  transcribe route opts into its own higher per-route `DefaultBodyLimit` (as
  `file/routes.rs` and `code_sandbox/routes.rs` do), so no global change is
  needed and other routes are unaffected.
- **Browser support:** `getUserMedia`/`MediaRecorder` is greenfield (grep-clean);
  no existing audio code to collide with. Unsupported browsers/denied permission
  degrade to a disabled mic button (no regression to the composer).

## Pattern conformance

- ITEM-1/2 mirror `build_helper/pgvector.rs` (build-from-source + `write_stubs`)
  fused with `build_helper/biomcp.rs` + `bio_mcp/embedded.rs` (per-triple
  `include_bytes!` + `*_available()` + extract-on-first-use). Triple set kept
  identical to `mcp/utils/embedded.rs`. Conformant.
- ITEM-3 mirrors `llm_local_runtime/engine/download.rs::download_file` (streaming
  + cap + progress) and `llm_model/storage.rs` cache-dir discipline; deliberately
  avoids the git-LFS/HF-repo path (documented mismatch: whisper `ggml-*.bin` is
  not GGUF and is a direct-URL single file). Conformant with an explicit,
  justified deviation.
- ITEM-4..10 mirror `web_search/` (module skeleton, `PermissionCheck` structs,
  singleton `id BOOLEAN PK CHECK(id=TRUE)` settings, COALESCE-patch repo, `api_route`
  + `*_docs` describers, `with_permission::<Perms>`, `sync_publish`, create/grant
  migration pair 97/98). Conformant.
- ITEM-8 mirrors `file/handlers/upload.rs` (Multipart, magic sniff, per-route body
  limit + logical cap) and the subprocess hardening from `bio_mcp/supervisor.rs`
  (`env_clear` + allow-list + `PR_SET_PDEATHSIG` + `kill_on_drop`). Conformant.
- ITEM-12/13 mirror the extension-slot composer architecture: a new
  `extensions/voice/extension.tsx` into `toolbar_actions`, text via `TextStore`,
  upload via `File.store.ts` FormData idiom. Never calls `sendMessage`.
  Conformant.
- ITEM-14 mirrors an existing settings module (`SettingsPageContainer`,
  `settingsAdminPages`, `sync:<entity>` self-gated refetch). Conformant.

## Migration collisions

`ls migrations/` — highest existing migration is
`00000000000131_rewrite_hub_ids_phibya_to_ziee_ai.sql`. The plan uses **132**
(`create_voice`) and **133** (`grant_voice_permissions_to_users`) — both free, no
collision. Naming follows the 14-digit zero-padded prefix + snake_case
convention, and the create/grant split mirrors 97/98, 100/101, 103/104. Grant
targets the system default `Users` group idempotently (`NOT (perm = ANY(...))`),
matching migration 98. `voice_settings` uses the singleton PK idiom already
present in `web_search_settings`/`code_sandbox_settings`.

## OpenAPI regen

Required. ITEM-8/9 add new typed endpoints (`Voice.transcribe`,
`Voice.getSettings`/`updateSettings`, `Voice.modelStatus`/`modelDownload`) and new
`JsonSchema` DTOs, plus the `VoiceSettings` sync entity. `just openapi-regen`
regenerates BOTH `ui/` and `desktop/ui/` `openapi.json` + `api-client/types.ts`;
the golden `openapi::emit_ts::tests::types_ts_parity` test fails if the committed
`types.ts` drifts, so regen is mandatory and verified. The generated files are
excluded from the phase-6 audit-coverage law and do not make the diff read as
"UI work" on their own (real hand-written `ui/**` files in ITEM-12..15 do, which
is why the plan carries e2e tests). `RequirePermissions`-gated handlers emit the
403 + bearerAuth security in the spec via `with_permission::<Perms>(op)`.

---

## Per-item verdicts

- **ITEM-1** — verdict: CONCERN — adds a cmake/C++ build-host dependency; acceptable because pgvector already assumes a compiler and the helper is fail-soft (stub on failure). Pin the submodule to a fixed tag; confirm `whisper-cli` builds statically (no runtime shared-lib deps to also embed).
- **ITEM-2** — verdict: PASS — direct clone of `bio_mcp/embedded.rs`; keep the cfg triple arms byte-synced with the build helper's `match` and `mcp/utils/embedded.rs`.
- **ITEM-3** — verdict: CONCERN — no existing generic direct-URL model downloader; must be written fresh from `download_file` idioms. sha256 pinning depends on stable known-model hashes (record them in an in-code table, verify after fetch); HF has no per-file `.sha256` sidecar.
- **ITEM-4** — verdict: PASS — mirrors `web_search/permissions.rs`; `voice::transcribe` + `voice::admin::{read,manage}`.
- **ITEM-5** — verdict: PASS — migrations 132/133 free; singleton + grant idioms already in-tree; `cargo clean` needed once for build.rs to pick up new migrations.
- **ITEM-6** — verdict: PASS — `Option<VoiceConfig>{enabled}` mirrors `WebSearchConfig`; additive, backward-compatible.
- **ITEM-7** — verdict: PASS — `web_search/` skeleton is the direct template; `MODULE_ENTRIES` order chosen after `mcp` (65) if it ever needs `mcp_servers` (it does not — voice is not an MCP server — so order is unconstrained; pick a free order slot).
- **ITEM-8** — verdict: CONCERN — needs `just openapi-regen` (new endpoint) + careful subprocess hardening + temp-file cleanup + WAV validation; whisper-cli output parsing (stdout JSON vs `-of` file) must be pinned against the vendored tag's actual CLI flags.
- **ITEM-9** — verdict: CONCERN — needs `just openapi-regen` (new endpoints + `VoiceSettings` sync entity); model-download endpoint may be long-running (base ≈ 142 MB) — resolve blocking-vs-async in DECISIONS.
- **ITEM-10** — verdict: CONCERN — mechanical but mandatory; must run in BOTH workspaces and keep the `types_ts_parity` golden green ([[project_openapi_regen_both_binaries]]).
- **ITEM-11** — verdict: CONCERN — macOS Tauri needs `NSMicrophoneUsageDescription` + entitlement or `getUserMedia` silently fails on desktop; Windows WebView2 prompt behavior to be verified on the Windows build host. Platform-gated, non-blocking for the web build.
- **ITEM-12** — verdict: PASS — extension-slot architecture supports this cleanly; in-browser resample-to-16 kHz-WAV keeps the server ffmpeg-free.
- **ITEM-13** — verdict: PASS — reuses the existing toolbar icon-button + tooltip idiom; unique `data-testid` required across both UI trees (enforced by the build).
- **ITEM-14** — verdict: PASS — standard settings-page module; `sync:voice_settings` self-gated refetch per the no-403 rule.
- **ITEM-15** — verdict: CONCERN — new render states must have gallery cells or `check:state-matrix` (inside `npm run check`) and the UI gate fail; budget the gallery entries with the states.
