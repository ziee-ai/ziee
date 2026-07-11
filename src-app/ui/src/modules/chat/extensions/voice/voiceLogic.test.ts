import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  appendTranscript,
  composeInterimCaption,
  isSuperseded,
  micErrorMessage,
  resolveLivePref,
  shouldRunInterim,
} from './voiceLogic.ts'

// ── "insert, don't send": a transcript APPENDS to the composer, never replaces ─
// The full store flow (record → transcribe → this append; and that sendMessage
// is never called) is covered by the 14-voice Playwright specs; this locks the
// pure append rule the store delegates to.

test('appendTranscript appends onto existing composer text, space-joined', () => {
  assert.equal(appendTranscript('Hello', 'world'), 'Hello world')
})

test('appendTranscript does NOT replace existing text', () => {
  // Regression guard for the "insert not overwrite" contract.
  assert.notEqual(appendTranscript('draft in progress', 'new words'), 'new words')
  assert.equal(appendTranscript('draft in progress', 'new words'), 'draft in progress new words')
})

test('appendTranscript into an empty composer yields just the transcript', () => {
  assert.equal(appendTranscript('', 'hello there'), 'hello there')
})

test('appendTranscript trims the transcript and is a no-op for blank speech', () => {
  assert.equal(appendTranscript('kept', '   '), 'kept', 'blank transcript leaves text unchanged')
  assert.equal(appendTranscript('kept', ''), 'kept')
  assert.equal(appendTranscript('a', '  b  '), 'a b', 'surrounding whitespace is trimmed')
})

// ── generation-token guard: a superseded result is dropped ────────────────────

test('isSuperseded is false while the request is still current (result kept)', () => {
  assert.equal(isSuperseded(5, 5), false)
})

test('isSuperseded is true once a cancel/newer request bumped the token (result dropped)', () => {
  assert.equal(isSuperseded(5, 6), true)
  assert.equal(isSuperseded(0, 1), true)
})

// ── getUserMedia rejection → user-facing error classification ─────────────────

test('micErrorMessage maps a permission denial to the "denied" message', () => {
  for (const name of ['NotAllowedError', 'SecurityError']) {
    const msg = micErrorMessage(new DOMException('nope', name))
    assert.match(msg, /denied/i, `${name} should be a permission denial`)
    assert.match(msg, /allow it in your browser/i)
  }
})

test('micErrorMessage maps a non-permission failure to the no-microphone message', () => {
  const generic = micErrorMessage(new DOMException('boom', 'NotFoundError'))
  assert.match(generic, /no microphone available/i)
  // A plain Error (not a DOMException) is also treated as no-microphone.
  assert.match(micErrorMessage(new Error('whatever')), /no microphone available/i)
})

// ── streaming (live-caption) decision helpers (TEST-9) ────────────────────────

const cap = (streaming_enabled: boolean, stream_interval_ms = 1000) => ({
  streaming_enabled,
  stream_interval_ms,
})

test('shouldRunInterim is true ONLY while recording with streaming available + pref on', () => {
  assert.equal(shouldRunInterim('recording', cap(true), true), true)
  // Off in every other status even with everything else on.
  for (const s of ['idle', 'requesting', 'transcribing', 'error']) {
    assert.equal(shouldRunInterim(s, cap(true), true), false, `status ${s} must not run interim`)
  }
  // Off when the deployment doesn't offer streaming, or the pref is off, or no capability.
  assert.equal(shouldRunInterim('recording', cap(false), true), false, 'deployment streaming off')
  assert.equal(shouldRunInterim('recording', cap(true), false), false, 'device pref off')
  assert.equal(shouldRunInterim('recording', null, true), false, 'no capability')
})

test('resolveLivePref honors a stored value and otherwise follows streaming_enabled', () => {
  // Stored value wins.
  assert.equal(resolveLivePref('1', false), true, "stored '1' → on")
  assert.equal(resolveLivePref('0', true), false, "stored '0' → off")
  // Unset → default follows the deployment toggle (opt-out default).
  assert.equal(resolveLivePref(null, true), true, 'unset + streaming on → default on')
  assert.equal(resolveLivePref(null, false), false, 'unset + streaming off → default off')
  // Any unexpected stored value falls back to the default.
  assert.equal(resolveLivePref('yes', true), true, 'garbage stored → default')
})

test('composeInterimCaption trims and clears blanks', () => {
  assert.equal(composeInterimCaption('  hello world  '), 'hello world')
  assert.equal(composeInterimCaption('   '), '', 'blank decode clears the caption')
  assert.equal(composeInterimCaption(null), '')
  assert.equal(composeInterimCaption(undefined), '')
})
