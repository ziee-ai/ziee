# DECISIONS — local-voice-dictation

Every human/product input the implementation needs, resolved up front so the
build runs nonstop. Each resolution is my recommendation grounded in a codebase
convention or an external fact; the ones I most want the user to confirm are
flagged `⟵ CONFIRM` and are echoed in the SendMessage summary.

---

### DEC-1: How is the whisper.cpp engine delivered — download prebuilt per-triple (biomcp), build-from-source + embed (pgvector), or in-process FFI via `whisper-rs`?
**Resolution:** Build the `whisper-cli` executable from a **vendored git submodule** via cmake (statically linked) in `build_helper/whisper.rs`, **embed it per-triple with `include_bytes!`**, extract-on-first-use, and invoke it as a subprocess — i.e. pgvector's build-from-source half fused with biomcp's embed+extract+subprocess half. Reject the "download prebuilt per-triple" option because whisper.cpp publishes **no official Linux/macOS per-triple binaries** (only Windows zips + third-party wheels), making it unreliable. Reject `whisper-rs` in-process FFI as the primary path because a cargo-feature FFI can't deliver the *runtime* fail-soft the constraint demands (a missing toolchain would fail the whole build, not degrade to a disabled mic).
**Basis:** codebase (`build_helper/pgvector.rs` + `biomcp.rs` + `bio_mcp/embedded.rs`) + external fact (no official whisper.cpp Linux/macOS release binaries) + the hard fail-soft constraint. `whisper-rs` is noted as the considered alternative (simpler, in-process, lower latency) worth revisiting if in-process transcription is later preferred. ⟵ CONFIRM

### DEC-2: Whisper MODEL delivery — embed in the binary, or download-on-demand?
**Resolution:** Embed the **binary** (small, build-coupled) but **download the model** (large, data). The model manager fetches `ggml-<model>.bin` by direct URL from the pinned HF repo on demand, caches it under `get_app_data_dir()/voice-models/`, and detects a pre-staged file for air-gapped operators. Bundle **no** model in the binary.
**Basis:** convention — this is exactly the split the codebase already makes (binaries embedded via `build_helper`; LLM model files downloaded via `llm_model`), and it keeps the shipped binary small.

### DEC-3: Which whisper model is the default, and which are selectable?
**Resolution:** Default **`base`** (multilingual, ~142 MB) for the accuracy/speed/size balance on CPU. Admin-selectable set: `tiny` (~75 MB, fastest), `base`, `base.en` (English-only, more accurate for English at the same size), `small` (~466 MB, most accurate/slowest). Quantized variants can be added later.
**Basis:** convention/research — whisper.cpp model sizes; `base` is the standard on-device balance. ⟵ CONFIRM (default model + whether to offer `small`).

### DEC-4: How is the downloaded model integrity-verified (HF has no per-file `.sha256` sidecar)?
**Resolution:** Pin the **known sha256** of each offered `ggml-*.bin` in an in-code constant table and verify after download (abort + delete on mismatch); this matches ziee's verify-everything ethos. The pinned model version is fixed alongside the hashes.
**Basis:** convention — mirrors `code_sandbox/known_revisions.toml` and biomcp's mandatory-sha256 posture; there is no upstream sidecar to fetch.

### DEC-5: Audio capture + format pipeline — where is the audio decoded/resampled?
**Resolution:** Capture with `getUserMedia` + `MediaRecorder` in the browser, then **decode + resample to 16 kHz mono and encode a WAV `Blob` in the browser** (Web Audio API), and POST the WAV. The server writes it to a temp file and feeds `whisper-cli` directly. This keeps the **server ffmpeg-free** (whisper-cli reads 16 kHz WAV natively).
**Basis:** external fact (whisper-cli expects 16 kHz mono WAV) + convention (the server bundles no ffmpeg; browser Web Audio is universally available). ⟵ CONFIRM (accept browser-side resampling vs. adding a server-side decoder).

### DEC-6: Transcribe endpoint shape + permission gate.
**Resolution:** `POST /api/voice/transcribe`, `multipart/form-data` with field `file` (the WAV), gated by `RequirePermissions<(VoiceTranscribe,)>` (`voice::transcribe`, granted to the `Users` group by migration 133). Returns `{ text: string, language: string, duration_ms: number }`. Per-route `DefaultBodyLimit` above the 16 MB global; logical caps from settings.
**Basis:** convention — `file/handlers/upload.rs` multipart + `web_search` permission/`api_route` idioms.

### DEC-7: Module / permission / settings naming.
**Resolution:** Backend module **`voice`** (broad enough to house future voice features), permission `voice::transcribe` + `voice::admin::{read,manage}`, config section `voice`, singleton table `voice_settings`, frontend chat extension `voice`, admin page `/settings/voice` ("Voice Dictation").
**Basis:** convention — matches the feature name and the `<module>::use`/`<module>::admin::*` permission convention; `voice` leaves room for the out-of-scope future modes without a rename.

### DEC-8: Push-to-talk interaction model — hold-to-talk or click-to-toggle?
**Resolution:** **Click-to-toggle** (click to start, click to stop) with a visible recording indicator (pulsing dot + elapsed timer) and a cancel/discard affordance. Hold-to-talk is rejected for v1 as hostile to keyboard/switch-access a11y.
**Basis:** convention/a11y — toggle is keyboard-operable and screen-reader-friendly (`aria-pressed`); a global hotkey can be added later via the existing keyboard extension. ⟵ CONFIRM (toggle vs hold).

### DEC-9: Transcript insertion behavior.
**Resolution:** **Append** the transcript to the current composer draft (space-joined if non-empty) via `Stores.Chat.TextStore.getText()/setText()`; **never** auto-send. The user reviews and presses Send.
**Basis:** user requirement (review-before-send) + convention (`TextStore` is the composer's text broker; `sendMessage` is never called).

### DEC-10: Language — auto-detect or explicit, per-recording or global?
**Resolution:** A **deployment/admin default language** in `voice_settings.language` (`auto` by default, whisper's auto-detect), applied to every transcription in v1. Per-recording language override in the composer is deferred (noted future) to keep the mic UI minimal.
**Basis:** convention — settings singleton holds deployment defaults; keeps the composer uncluttered. ⟵ CONFIRM (global default acceptable for v1).

### DEC-11: CPU vs GPU whisper, and latency/caps.
**Resolution:** **CPU-only** whisper build for v1 (portable, no CUDA/Metal per-backend build matrix; matches the self-host/air-gap posture). GPU deferred (whisper.cpp CUDA/Metal is a per-backend build like llama.cpp). Caps: `max_clip_seconds` default **120**, `max_upload_bytes` default **32 MB** (a 120 s 16 kHz mono WAV is ≈ 3.8 MB, so 32 MB is generous headroom); the transcribe route layers `DefaultBodyLimit::max(32 MB)`.
**Basis:** convention/research — CPU keeps the delivery simple and universal; caps sized from the WAV bitrate. Expected latency: `base` on CPU transcribes a short clip in ~1–3 s plus a one-time model load per subprocess invocation.

### DEC-12: Model-management depth for v1 — lazy-on-first-use vs explicit admin download; SSE progress?
**Resolution:** **Explicit admin "Download model" action** on the settings page (`POST /api/voice/model/download`) + a `GET /api/voice/model/status`; the mic button is disabled until a model is present. Transcribe does **not** trigger a large blocking download inside the request. SSE download progress is **deferred** (a simple pending/complete status suffices for v1); the streaming downloader already reports progress internally if we later wire SSE.
**Basis:** convention — avoids a multi-hundred-MB blocking fetch on a user's first mic tap; mirrors `code_sandbox` admin pre-fetch. ⟵ CONFIRM (explicit admin download vs lazy-on-first-use; SSE in/out).

### DEC-13: Desktop native microphone permission.
**Resolution:** Add macOS `NSMicrophoneUsageDescription` (+ the microphone entitlement) to the Tauri config so `getUserMedia` prompts correctly in the macOS webview; rely on WebView2's built-in prompt on Windows (verify on the Windows build host). The voice extension + settings page **ship on desktop** (server is embedded) — not added to `CORE_MODULE_BLOCKLIST`.
**Basis:** convention/platform — Tauri/macOS requires the usage-description string or `getUserMedia` fails silently; `project_desktop_embeds_server` means the embedded server already carries whisper.

### DEC-14: How does the integration transcribe test (TEST-7) avoid a green-washing skip?
**Resolution:** Pre-stage a **`tiny`** model on the Linux CI/build host (small, ~75 MB) and ship a short fixture WAV so TEST-7 runs the **real embedded whisper-cli** for real. The `#[ignore]`-style gate is only a fallback for a stub-whisper build (genuine asset unavailability), never used to make a red suite green.
**Basis:** convention ([[feedback_no_ignore_unless_platform]] + [[feedback_no_cosmetic_tests]]) — the transcription claim is backed by a real-path test; only the external model asset is staged.

### DEC-15: `MODULE_ENTRIES` ordering for the `voice` backend module.
**Resolution:** Register at a free `order` slot with no cross-module dependency (voice is **not** an MCP server and needs no `mcp_servers` row), so ordering is unconstrained — pick the next free slot after the built-in feature modules.
**Basis:** codebase — unlike `web_search` (order 96, after `mcp` at 65 because it upserts an MCP row), voice has no such dependency.
