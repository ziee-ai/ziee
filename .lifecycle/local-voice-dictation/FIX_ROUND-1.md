# FIX_ROUND-1 — fix the phase-6 ledger, then re-audit

## Fixes applied (all 21 confirmed/actionable phase-6 findings)

Backend (`b9d19bd48`):
- **[HIGH] health state machine inert from persisted `stopped`** — added `HealthStateMachine::mark_starting()` (moves any non-`Failed` state to `Starting`) called at `auto_start::do_start`; breaks the `Stopped` absorbing wedge so StartedOk→Healthy and later Crashed advances the flap window. New tests `persisted_stopped_restarts_then_flaps_to_failed` + `mark_starting_does_not_disturb_failed`.
- **[MED] runtime death mis-classified** — `LocalDeployment::poll_liveness()` (`try_wait`) → `reaper` emits `HealthEvent::Crashed` on real exit so a runtime crash-loop trips give-up.
- **[MED] partial-extract cache poisoning** — `remove_dir_all(cache_dir)` on any extract error.
- **[MED] download-task double-runner race** — `START_OR_JOIN_LOCK` around the check-and-replace.
- **[MED] body-limit vs settings cap** — migration `..133` + handler cap `max_upload_bytes` at 64 MiB (consistent with the route limit).
- **[MED] blocking file I/O in async model download** — switched to `tokio::fs` async writes.
- **[LOW] fail-open on unpinned model** — verification now fails closed (reject missing/placeholder pin).
- **[LOW] language not validated** — allow-list (auto/empty/ISO-639-1) at settings-set time.
- **[LOW] multipart Err arm masked** — explicit match → 400 `VOICE_BAD_UPLOAD`.
- **[LOW] avoidable audio copy** — zero-copy `Bytes` forward via `Part::stream_with_length`.

Frontend (`b9d19bd48`):
- **[HIGH] mic stream leaked on unmount** — `useEffect` unmount cleanup → `cancelRecording()` (stops tracks + timers).
- **[MED] unescapable `requesting`** — `requestGeneration` token + 15 s timeout + a Cancel button.
- **[MED] 403-storm** — `VoiceDownloadProgress.loadActive` self-gates on `VoiceAdminRead`.
- **[MED] a11y focus / live-region / error** — focus return to composer, an `announcement`-driven live region, error surfaced to AT.
- **[MED] not-ready guidance unreachable** — focusable `aria-disabled` button + `aria-describedby` remediation.
- **[MED] raw backend error leaked** — clean toast, raw detail to console only.
- **[LOW] copy casing/grammar**, **[LOW] SSE double-subscribe** (sync placeholder), **wav tests** strengthened to real value assertions (5→9).

Test-coverage findings (5, tests-quality) are addressed in **phase 8** (integration `tests/voice/` + `MockReleaseServer` wiring `stub-whisper-server` + the strengthened wav tests already landed).

## Re-audit (round 1 → full blind round)

Two fresh blind agents (backend + frontend) re-reviewed `git diff origin/main...HEAD`. They confirmed all round-1 fixes correct EXCEPT three, and found new/incomplete defects:
1. **[HIGH]** Deploy kill switch `voice.enabled=false` bypassable — `register_routes` still merged the router (transcribe reachable, whisper-server spawnable, un-reaped).
2. **[MED]** The "single persistent" aria-live region was actually per-branch (remounted with text → not announced) — a11y fix structurally incomplete.
3. **[MED]** Transcribe-success path had no supersession guard — the mic-leak reappeared one state later (unmount during `transcribing` still appended into a left conversation).

**New confirmed findings:** 3
