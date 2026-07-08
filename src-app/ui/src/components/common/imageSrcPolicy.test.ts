import { test } from 'node:test'
import assert from 'node:assert/strict'
import { classifyImageSrc } from './imageSrcPolicy.ts'

// TEST-4: the image anti-exfil policy is UNCHANGED after ITEM-3 extracted it
// into a pure classifier. Regression guard that ReservedImage did not weaken
// the SSRF/exfil block. No DOM.

const ORIGIN = 'https://app.ziee.test'

test('empty / non-string src → empty (render nothing)', () => {
  assert.equal(classifyImageSrc('', ORIGIN), 'empty')
  assert.equal(classifyImageSrc(undefined, ORIGIN), 'empty')
  assert.equal(classifyImageSrc(null, ORIGIN), 'empty')
  assert.equal(classifyImageSrc(123, ORIGIN), 'empty')
})

test('root-relative and same-origin absolute src → allowed', () => {
  assert.equal(classifyImageSrc('/api/files/x/download', ORIGIN), 'allowed')
  assert.equal(classifyImageSrc(`${ORIGIN}/api/files/y`, ORIGIN), 'allowed')
})

test('external origin src → blocked (exfil beacon vector)', () => {
  assert.equal(classifyImageSrc('https://exfil.test/?token=abc', ORIGIN), 'blocked')
  assert.equal(classifyImageSrc('http://evil.example/pixel.gif', ORIGIN), 'blocked')
  // Protocol-relative URL resolves to a DIFFERENT origin → blocked.
  assert.equal(classifyImageSrc('//evil.test/pixel.gif', ORIGIN), 'blocked')
})

test('backslash-disguised authority → blocked (the URL parser folds \\ to /)', () => {
  // `/\evil.test` starts with a single `/` (so a naive startsWith('/') fast-path
  // would allow it) but the WHATWG parser resolves it to https://evil.test — the
  // origin check catches it.
  assert.equal(classifyImageSrc('/\\evil.test/pixel.gif', ORIGIN), 'blocked')
  assert.equal(classifyImageSrc('\\\\evil.test/pixel.gif', ORIGIN), 'blocked')
})

test('same-origin relative forms stay allowed', () => {
  // A single leading backslash resolves same-origin (harmless) → allowed.
  assert.equal(classifyImageSrc('\\evil.test', ORIGIN), 'allowed')
  // Bare relative path → same origin → allowed.
  assert.equal(classifyImageSrc('image.png', ORIGIN), 'allowed')
})

test('data: URI → blocked', () => {
  assert.equal(
    classifyImageSrc('data:image/png;base64,AAAA', ORIGIN),
    'blocked',
  )
})

test('opaque-scheme and throwing URLs → blocked (never accidentally allowed)', () => {
  // javascript: parses but has an opaque (null) origin → blocked.
  assert.equal(classifyImageSrc('javascript:alert(1)', ORIGIN), 'blocked')
  // Absolute-with-empty-host throws in the URL parser → blocked.
  assert.equal(classifyImageSrc('http://', ORIGIN), 'blocked')
})
