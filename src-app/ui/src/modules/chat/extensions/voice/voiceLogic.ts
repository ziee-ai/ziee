/**
 * Pure decision helpers for the voice-dictation state machine, extracted from
 * `Voice.store.ts` so they can be unit-tested without the store's browser +
 * `defineExtensionStore` graph (the store itself is exercised end-to-end by the
 * `14-voice/` Playwright specs). Each encodes one load-bearing contract:
 *
 *  - `appendTranscript` — the "insert, don't send" rule: a transcript is
 *    APPENDED to the existing composer text (space-joined), never replacing it
 *    and — by construction — never triggering a send.
 *  - `isSuperseded` — the generation-token guard: a result whose request was
 *    superseded (by cancel / unmount / a newer request) must be DROPPED.
 *  - `micErrorMessage` — maps a `getUserMedia` rejection to the user-facing
 *    error, distinguishing a permission denial from a no-microphone failure.
 */

/**
 * Compose the next composer value after a transcription. Appends (never
 * replaces), space-joined onto any existing text; a blank transcript is a no-op.
 * There is deliberately NO send path here — dictation only ever inserts.
 */
export function appendTranscript(current: string, transcript: string): string {
  const text = transcript.trim()
  if (!text) return current
  return current ? `${current} ${text}` : text
}

/**
 * True when the generation token captured at a request's start no longer
 * matches the current token — i.e. a cancel/unmount/newer-request superseded it,
 * so its (recording or transcription) result must be discarded.
 */
export function isSuperseded(genAtStart: number, currentGen: number): boolean {
  return genAtStart !== currentGen
}

/**
 * User-facing error for a `getUserMedia` rejection. A `NotAllowedError` /
 * `SecurityError` `DOMException` is a permission denial; anything else is a
 * missing/unavailable microphone.
 */
export function micErrorMessage(err: unknown): string {
  const denied =
    err instanceof DOMException &&
    (err.name === 'NotAllowedError' || err.name === 'SecurityError')
  return denied
    ? 'Microphone access was denied. Allow it in your browser to dictate.'
    : 'Could not start recording — no microphone available.'
}

// ── streaming (live-caption) decision helpers ─────────────────────────────────
// Pure logic for the interim loop, extracted so the store's cadence/gate is
// unit-testable without the browser + MediaRecorder graph.

/** The subset of `VoiceCapability` the interim decision needs (avoids a store cycle). */
export interface InterimCapability {
  streaming_enabled: boolean
  stream_interval_ms: number
}

/**
 * True when the composer should run the live-caption interim loop: only while
 * actively `recording`, only when the deployment offers streaming captions, and
 * only when this device's user pref has them ON. Any other status (idle,
 * requesting, transcribing, error) must NOT decode interim frames.
 */
export function shouldRunInterim(
  status: string,
  capability: InterimCapability | null | undefined,
  livePref: boolean,
): boolean {
  return status === 'recording' && !!capability?.streaming_enabled && livePref
}

/**
 * Resolve the per-device "Live captions" preference. A stored `'1'`/`'0'` wins;
 * with nothing stored the default FOLLOWS the deployment `streaming_enabled`
 * (on when available — the opt-out default, DEC-11). Anything else → the default.
 */
export function resolveLivePref(stored: string | null, streamingEnabled: boolean): boolean {
  if (stored === '1') return true
  if (stored === '0') return false
  return streamingEnabled
}

/**
 * Normalize an interim transcript into the live caption. Trimmed; a blank decode
 * clears the caption (empty string) rather than showing whitespace.
 */
export function composeInterimCaption(text: string | null | undefined): string {
  return (text ?? '').trim()
}
