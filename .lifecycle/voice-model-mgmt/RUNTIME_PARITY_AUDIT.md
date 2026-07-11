# Voice runtime vs llm_local_runtime — consolidated parity audit (FB-2)

Three blind parity-audit agents (lifecycle/health/supervision · deployment/proxy/version ·
settings/errors/tests) compared `modules/voice/` against the mature `modules/llm_local_runtime/`.
Load-bearing lifecycle finding independently re-verified against source by the orchestrator (P1).

**Overall:** voice is a faithful single-engine/singleton port and on several axes is at parity or
AHEAD (mandatory sha256 fail-closed download verification the llm runtime LACKS — must NOT be
aligned down; more thorough settings validation; full 14-test health SM port). The gaps are a
handful of real correctness wiring bugs + missing surfaces + missing tests. None are data-corruption.

## Findings (severity, voice → llm mirror)

### Correctness / robustness
- **F1 [MED] Crash-after-healthy not counted on the request path** — `voice/auto_start.rs:308-345`
  (`live_handle_if_current` returns None on a running-but-dead row without feeding `Crashed`) vs
  `llm_local_runtime/auto_start.rs:381-418` + `probe_liveness:284-330`. The 5/60s flap cap only
  sees crashes via the 60s reaper poll (`reaper.rs:127`), so a "becomes-healthy-then-dies" loop
  respawns without give-up. **Bounded:** pre-healthy crashes ARE caught (do_start timeout →
  `Crashed`, auto_start.rs:340; `is_failed()` gate at :316). VERIFIED by orchestrator.
- **F2 [MED] Exponential backoff computed then discarded** — `voice/auto_start.rs` has no `next_at`
  gate and `mark_starting()` (:405) overwrites `Restarting` → restarts fire with no delay — vs
  `llm_local_runtime/auto_start.rs:399-405` (honors `next_at`). VERIFIED.
- **F3 [MED-HIGH] Model download not cancellable + temp leak on shutdown** — `voice/model.rs:131-271`
  (no cancel token, no `SHUTDOWN.notified()` race; abort skips the Err-branch `.tmp` cleanup at
  :223-227 → uuid `.tmp` leaks) vs `runtime_version/download_task.rs:303-310` (SHUTDOWN race) /
  `llm_model/handlers/uploads.rs:1056` (cancel token). **Directly in this feature's rework path.**
- **F4 [LOW-MED] Model-switch/activate kills in-flight transcription without draining** —
  `voice/auto_start.rs:382-419` + `deployment/local.rs:205-213` (start() stops prior process
  unconditionally) — no drain on model change. **Directly in this feature's new activate-model path.**
- **F5 [LOW-MED] `forward_to_whisper` reqwest client omits `.no_proxy()`** — `voice/transcribe.rs:175-178`
  vs voice's own `health_check` (`deployment/local.rs:352`, sets it) + llm proxy
  (`proxy_handlers.rs:484`). Loopback inference could route through an env HTTP proxy; also a
  per-request client vs a shared pool.
- **F6 [LOW] No drain front-door (`Draining`) flag** — `voice/reaper.rs:177-194` vs
  `llm_local_runtime/reaper.rs:261-288` (sets `InstanceFlag::Draining`; proxy 503s new work). Voice
  drain is best-effort/racy. (Voice has no proxy front door — mitigation would be a transcribe-path gate.)
- **F7 [LOW] `voice_runtime_instance.state` lacks a `CHECK` constraint** — `migrations/151:45` vs
  `migrations/066:9-11`.

### Missing surfaces
- **F8 [MED-HIGH] No logs / logs-stream endpoint** — capture machinery fully built but dead-coded
  (`voice/deployment/local.rs:370-388` `#[allow(dead_code)]`) with NO route (`voice/routes.rs:19-48`)
  vs `llm_local_runtime/routes.rs:49-57` (`/logs` + `/logs/stream` SSE). Admin can't read a failing
  whisper-server's output. (Tracked in-repo as "deferred DRIFT-1".)
- **F9 [LOW-MED] No single-download poll-snapshot endpoint** — voice registers SSE-only
  (`runtime_version/mod.rs:39-45`); `snapshot_of` builder exists but no route — vs
  `llm_local_runtime/routes.rs:96-99`. No non-SSE fallback.
- **F10 [LOW] No `GET .../versions/{id}`, no `detect-gpu` route, live pid/uptime dead-coded on
  status** — minor surface deltas (`voice/instance_handlers.rs:68-90`, `deployment/local.rs:57-61`).

### Missing tests (≈5 categories)
- **F11** sync_emit (`tests/voice/` has zero `SyncEntity::` assertions) · granular reaper/drain ·
  engine-log SSE (tied to F8) · gold/e2e smoke · crash-supervision integration. (voice: 43 unit
  vs llm 61; health SM well-covered.)

## Explicitly NOT gaps (legitimate voice-vs-llm differences — do not "fix")
Same-port OpenAI reverse proxy / token cache / bearer rewrite (voice isn't a chat provider — direct
authenticated `POST /voice/transcribe`); typed per-engine `engine_settings` vocabulary (single fixed
-flag whisper engine); `version-usage` grouping + `MAX_CONCURRENT_ENGINES` (hard singleton);
`allow_unsigned_downloads` (intentionally dropped). Voice is AHEAD on download verification.

## Proposed scope split (for the model-mgmt feature)
- **Tier A — fold in (on paths THIS feature already builds/reworks; cheap correctness):** F3
  (async model-download task already mirrors the SHUTDOWN-race runtime_version pattern → cancel +
  temp-cleanup for free), F4 (drain before activate-respawn), F7 (state CHECK in migration 155),
  F5 (`.no_proxy()` one-liner), F9 (poll snapshot for the new model download), + F11 sync_emit &
  drain tests for the touched paths.
- **Tier B — recommend folding (runtime supervision correctness):** F1 + F2 (feed `Crashed` on a
  running-but-dead row in `ensure_running`, enforce backoff `next_at`) + a supervision test + F6.
- **Tier C — separate branch (distinct "expose voice logs" feature):** F8 logs + logs-stream (+
  its SSE test), F10 minor surfaces.
