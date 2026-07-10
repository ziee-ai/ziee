# DECISIONS — local-voice-dictation (full managed whisper runtime)

Every human/product input the implementation needs, resolved up front. Each is my
recommendation grounded in a codebase convention or an external fact; the ones I
most want confirmed are flagged `⟵ CONFIRM`.

---

### DEC-1: Whisper engine delivery — build-at-`build.rs`+embed, upstream prebuilt, or a ziee fork + download-on-demand?
**Resolution:** **A ziee-owned `ziee-ai/whisper.cpp` fork whose CI builds `whisper-server` from source per `{platform}-{arch}-{backend}` and publishes GitHub Releases; the ziee server downloads on demand at runtime** — the exact model already used for `ziee-ai/llama.cpp` / `ziee-ai/mistral.rs`. Reject "download upstream prebuilt" (whisper.cpp ships **no** official Linux/macOS per-triple binaries). Reject "build at `build.rs` time + embed" because a build host can only compile its **own** triple, so an embedded binary would silently fail on every other platform; it also forces cmake onto every ziee build and bloats the binary.
**Basis:** codebase (`llm_local_runtime/engine/download.rs`, `ziee-ai/llama.cpp`) + external fact (no official binaries; cross-triple builds infeasible at `build.rs` time). This supersedes the earlier build-and-embed plan.

### DEC-2: Scope — one-shot `whisper-cli` per clip, or a FULL managed runtime like `llm_local_runtime`?
**Resolution:** **Full managed runtime.** Run `whisper-server` as a managed, long-lived instance with the complete lifecycle mirrored from `llm_local_runtime`: version registry + download + update flow + admin UI + runtime settings + health state machine + idle-reaper + drain + crash-restart. The transcribe endpoint forwards to the loopback instance.
**Basis:** user directive ("we need a full feature; check how llm runtime is managed, download, update"). `whisper-server`'s HTTP mode makes a managed instance natural. ⟵ CONFIRM (full runtime is a large surface — confirm the scope vs a leaner one-shot `whisper-cli`).

### DEC-3: Instance topology — per-model instances (like llm runtime) or a single hot-swappable instance?
**Resolution:** **A single managed `whisper-server` instance** loaded with the admin-configured model; a model change drains+restarts (or uses whisper-server's model hot-swap). Whisper transcribes with one model at a time, so the per-model fan-out of `llm_local_runtime` is unnecessary complexity.
**Basis:** external fact (whisper-server hot-swaps a single model) + simplification. Keeps `auto_start`/`reaper` to one instance key.

### DEC-4: Code reuse — extend `llm_local_runtime`'s `EngineType`, or copy-adapt into a new `voice` module?
**Resolution:** **Copy-adapt** the engine/version/health/reaper/download patterns into a self-contained `modules/voice/`. Do **not** add `EngineType::Whisper` to `llm_local_runtime` — whisper is a speech runtime, not an LLM inference engine; coupling it into the provider/model machinery would be wrong.
**Basis:** codebase separation of concerns; the research recommended "mirror this layout," not "extend it."

### DEC-5: Both binary AND model downloaded — nothing embedded?
**Resolution:** **Yes.** The `whisper-server` **binary** downloads like the LLM engines (fork releases), and the ggml **model** downloads like the LLM model files (direct HF URL). Nothing whisper-sized is baked into the ziee binary. One consistent story.
**Basis:** convention — matches the binary-vs-model split already in the codebase; keeps the shipped binary small.

### DEC-6: Default model + selectable set + integrity verification.
**Resolution:** Default **`base`** (multilingual, ~142 MB) for the CPU accuracy/speed/size balance; selectable `tiny` / `base` / `base.en` / `small`. Verify each downloaded `ggml-*.bin` against a **pinned in-code sha256 table** (HF has no per-file sidecar); engine binaries verify against the fork release's `.sha256` sidecar (cosign `.sig` slot reserved, matching the LLM runtime's TOFU-today posture).
**Basis:** research (whisper model sizes) + ziee's verify-everything ethos (`code_sandbox/known_revisions`). ⟵ CONFIRM (default model + whether to offer `small`).

### DEC-7: Audio capture + format pipeline.
**Resolution:** Capture with `getUserMedia`+`MediaRecorder`, then **decode+resample to 16 kHz mono and encode WAV in the browser** (Web Audio API); POST the WAV; the server forwards it to whisper-server `/inference`. Keeps the **server ffmpeg-free** (whisper-server needs 16 kHz mono PCM).
**Basis:** external fact (whisper-server input requirement) + convention (no ffmpeg bundled). ⟵ CONFIRM (browser-side resample vs a server-side decoder).

### DEC-8: whisper-server request path — native `/inference` or OpenAI `/v1/audio/transcriptions`?
**Resolution:** Use whisper-server's **native `/inference`** with `response_format=json` and the `language` param; the OpenAI-compatible `/v1/audio/transcriptions` endpoint is available but the native one is the stable primary. Pin the exact request/response shape against the fork's tagged `whisper-server --help`.
**Basis:** external fact (whisper-server exposes both); native endpoint is the documented primary.

### DEC-9: Module / permission / settings naming.
**Resolution:** Backend module **`voice`** (with internal `engine/`, `runtime_version/`, `runtime_settings/`, `model/`, `deployment/` submodules mirroring `llm_local_runtime`); permissions `voice::transcribe` + `voice::admin::{read,manage}`; config section `voice`; tables `voice_runtime_versions` / `voice_runtime_instance` / `voice_runtime_settings`; frontend chat extension `voice` + admin module `voice`; admin page `/settings/voice` ("Voice Dictation").
**Basis:** convention — matches the feature name; `voice` leaves room for the out-of-scope future modes without a rename.

### DEC-10: Permission granularity — the llm runtime's 9-perm split or a `use`+`admin::{read,manage}` split?
**Resolution:** **`voice::transcribe` (users) + `voice::admin::{read,manage}` (admins)** — the cleaner web_search-style split. Admin is auto-covered by the Administrators `*` wildcard; only `voice::transcribe` needs an explicit `Users` grant (migration 152).
**Basis:** convention — `web_search`/`lit_search` use this modern split; the llm runtime's finer split is historical.

### DEC-11: Push-to-talk interaction model.
**Resolution:** **Click-to-toggle** (click start / click stop) with a recording indicator (pulsing dot + elapsed timer) and a cancel/discard affordance. Hold-to-talk rejected for v1 (hostile to keyboard/switch a11y).
**Basis:** a11y/convention — toggle is keyboard-operable (`aria-pressed`); a hotkey can come later via the keyboard extension. ⟵ CONFIRM (toggle vs hold).

### DEC-12: Transcript insertion.
**Resolution:** **Append** to the current composer draft (space-joined) via `Stores.Chat.TextStore.getText()/setText()`; **never** auto-send.
**Basis:** user requirement (review-before-send) + convention (TextStore is the composer text broker).

### DEC-13: Language — auto-detect vs explicit; global vs per-recording.
**Resolution:** A single **deployment default** `voice_runtime_settings.language` (`auto` by default), applied to every transcription in v1; per-recording override deferred (keeps the mic UI minimal).
**Basis:** convention (settings singleton holds deployment defaults). ⟵ CONFIRM (global default acceptable for v1).

### DEC-14: CPU vs GPU + resource caps.
**Resolution:** **CPU backend only** in v1 (the fork's matrix reserves cuda/metal/vulkan slots for later; the runtime's `{backend}` axis + `recommend_backend` are wired but only `cpu` publishes/downloads). Caps: `max_clip_seconds` default **120**, `max_upload_bytes` default **32 MB**; transcribe route layers a matching `DefaultBodyLimit`. Idle-unload default **1800 s**; auto-start/drain timeouts **30 s** (mirroring the llm runtime defaults).
**Basis:** convention/research — CPU keeps delivery universal; caps sized from the 16 kHz WAV bitrate (~3.8 MB for 120 s); timeouts copied from `llm_runtime_settings`.

### DEC-15: Model & version management UX depth — SSE progress in v1?
**Resolution:** **Yes — full SSE progress**, because we're mirroring the llm runtime's detached download-task + `RuntimeDownloadProgress` SSE consumer (reload-safe). Admin installs/updates engine versions and downloads models from `/settings/voice` with live `<Progress>` bars; the mic button is disabled until a model + a runtime binary are present.
**Basis:** convention — reuse `runtime_version/download_task.rs` + the frontend SSE store rather than inventing a lesser progress mechanism.

### DEC-16: Fail-soft trigger set.
**Resolution:** The mic self-disables + the module logs "voice disabled" when ANY of: `voice.enabled=false` (deploy kill switch), no runtime binary resolvable (no release/asset, air-gap not staged), no model present, or the browser lacks `getUserMedia`/permission. The server always boots; admin surfaces stay reachable so an admin can fix (download binary/model).
**Basis:** hard requirement (fail-soft like pgvector/biomcp) + convention (config gate returns early in `init()`).

### DEC-17: Air-gapped operation.
**Resolution:** Support pre-staging **both** the `whisper-server` binary (into the version cache dir) and the ggml model (into `voice-models/`); the runtime detects staged assets and skips all network. Mirrors the sandbox-rootfs + LLM-model air-gap stories.
**Basis:** convention + the LOCAL/air-gap positioning.

### DEC-18: How does the real-transcription integration test avoid a green-washing skip?
**Resolution:** Pre-stage a **`tiny`** model + a real (fork-built or locally-built) `whisper-server` on the Linux CI/build host + ship a short fixture WAV, so TEST-11 runs the **real** managed instance for real. The `#[ignore]` gate is only a fallback for a genuine stub-binary/model-absent build, never used to hide a red suite. Version/download/update/lifecycle tests use `stub-whisper-server` + `MockReleaseServer` (real code paths, mocked release/model hosts only).
**Basis:** convention ([[feedback_no_ignore_unless_platform]], [[feedback_no_cosmetic_tests]]) — the transcription claim is backed by a real-path test.

### DEC-19: Desktop native microphone permission.
**Resolution:** Add macOS `NSMicrophoneUsageDescription` (+ mic entitlement) to the Tauri config so `getUserMedia` prompts in the macOS webview; rely on WebView2's prompt on Windows (verify on the Windows build host). The voice extension + admin page **ship on desktop** (server embedded) — not added to `CORE_MODULE_BLOCKLIST`.
**Basis:** platform requirement + `project_desktop_embeds_server`.

### DEC-20: `MODULE_ENTRIES` order for the `voice` backend module.
**Resolution:** Register at a free `order` slot with no cross-module dependency (voice needs no `mcp_servers` row and no provider ordering); a slot near the other runtime modules (~32–36 range, first free) is fine. `init()` spawns the reaper.
**Basis:** codebase — voice has no ordering constraint (unlike `web_search` after `mcp`).

### DEC-21: Mic button placement / discoverability.
**Resolution:** An **always-visible** `Mic` icon in the composer `toolbar_actions` slot (next to `+`), not buried in the `+` menu — dictation is a flagship, one-tap affordance.
**Basis:** convention/UX — flagship input affordances live inline in the toolbar; the `+` menu is for occasional actions. ⟵ CONFIRM (inline vs `+` menu).

### DEC-22: Normal-user experience when the feature is enabled but NOT provisioned (only admins can download runtime/model).
**Resolution:** Three distinct states driven by a non-admin `GET /api/voice/capability` (`{enabled, runtime_ready, model_ready, can_transcribe}`): (1) **ready** → mic enabled; (2) **enabled but runtime/model missing** → mic **disabled with tooltip "Voice dictation isn't set up yet — contact an administrator"** (discoverable, honest); (3) **feature/deploy flag off, or browser lacks `getUserMedia`/secure-context** → mic **hidden**. A normal user can NEVER trigger a model/runtime download (admin-gated) — no lazy multi-hundred-MB fetch on a mic tap.
**Basis:** convention/UX + the admin-only provisioning constraint; mirrors code_sandbox's "not installed" honesty. ⟵ CONFIRM (disabled-with-tooltip vs fully hidden when unprovisioned).

### DEC-23: Cold-start latency messaging.
**Resolution:** The transcribing UI shows a **staged status** — "Starting voice engine…" while the idle-unloaded `whisper-server` auto-starts + loads the model (first use after idle), then "Transcribing…" — via `aria-live` so it never looks hung.
**Basis:** UX — the managed instance's cold start can be several seconds; the LLM runtime has the same auto-start latency and surfaces status similarly.

### DEC-24: Maximum clip length handling.
**Resolution:** The recorder **auto-stops** at `max_clip_seconds` (default 120) with a visible countdown as it approaches and a "Reached maximum length" note; the captured audio is still transcribed (not discarded).
**Basis:** convention — matches the server-side `max_clip_seconds` cap; avoids a silent truncation surprise.

### DEC-25: Privacy reassurance (the local/air-gap differentiator).
**Resolution:** A **dismissible one-time first-use hint** near the mic: "Audio is transcribed locally on your server — never sent to the cloud." Reinforces ziee's self-hosted/private positioning at the exact moment of doubt.
**Basis:** product positioning ([[project_life_science_initiative]] privacy stance) — cheap trust signal at first use.

### DEC-26: Admin onboarding step + live volume meter — in v1 or deferred?
**Resolution:** **Defer both.** v1 relies on the settings-page empty-state banner + install/download cards for admin setup guidance (no dedicated getting-started "voice-setup" step yet), and a simple pulsing indicator + timer for recording (no live waveform/volume meter).
**Basis:** scope control — the settings empty-state already guides setup; a `memory`-style onboarding step and a waveform meter are additive polish, noted as future.
