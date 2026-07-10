import { test } from 'node:test'
import assert from 'node:assert/strict'
import { appendTranscript, isSuperseded, micErrorMessage } from './voiceLogic.ts'

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
