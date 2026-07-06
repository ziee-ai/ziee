// Tier 2 — manifest builder unit test. Run with: node --test scripts/updater/
//
// Drives the real build-latest-json.mjs against fixture artifacts and locks the
// contract the production updater depends on (the exact platform-key map shape).

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'

import {
  buildLatestJson,
  platformKeyForFilename,
  PLATFORM_KEYS,
} from './build-latest-json.mjs'

function makeFixtureDir(version = '0.2.0') {
  const dir = mkdtempSync(path.join(tmpdir(), 'ziee-updater-fixtures-'))
  const files = {
    'darwin-aarch64': `ziee_${version}_darwin-aarch64.app.tar.gz`,
    'darwin-x86_64': `ziee_${version}_darwin-x86_64.app.tar.gz`,
    'linux-x86_64': `ziee_${version}_linux-x86_64.AppImage`,
    'windows-x86_64': `ziee_${version}_windows-x86_64.msi`,
  }
  const sigs = {}
  for (const [key, name] of Object.entries(files)) {
    writeFileSync(path.join(dir, name), `dummy-bundle-${key}`)
    // base64-ish placeholder signature (Tier 3 covers real crypto).
    const sig = Buffer.from(`signature-for-${key}`).toString('base64')
    sigs[key] = sig
    writeFileSync(path.join(dir, `${name}.sig`), sig + '\n')
  }
  return { dir, files, sigs }
}

test('platformKeyForFilename matches each platform key', () => {
  assert.equal(platformKeyForFilename('ziee_0.2.0_darwin-aarch64.app.tar.gz'), 'darwin-aarch64')
  assert.equal(platformKeyForFilename('ziee_0.2.0_darwin-x86_64.app.tar.gz'), 'darwin-x86_64')
  assert.equal(platformKeyForFilename('ziee_0.2.0_linux-x86_64.AppImage'), 'linux-x86_64')
  assert.equal(platformKeyForFilename('ziee_0.2.0_windows-x86_64.msi'), 'windows-x86_64')
  assert.equal(platformKeyForFilename('something-unrelated.txt'), null)
})

test('buildLatestJson produces the exact static-manifest shape', () => {
  const { dir, files, sigs } = makeFixtureDir('0.2.0')
  try {
    const m = buildLatestJson({
      artifactsDir: dir,
      baseUrl: 'https://github.com/ziee-ai/ziee/releases/download/v0.2.0',
      version: '0.2.0',
      notes: 'Release notes here',
      pubDate: '2026-06-11T00:00:00.000Z',
    })

    assert.equal(m.version, '0.2.0')
    assert.equal(m.notes, 'Release notes here')
    assert.equal(m.pub_date, '2026-06-11T00:00:00.000Z')

    // Platform map keyed EXACTLY by the four Tauri keys.
    assert.deepEqual(Object.keys(m.platforms).sort(), [...PLATFORM_KEYS].sort())

    for (const key of PLATFORM_KEYS) {
      assert.equal(m.platforms[key].signature, sigs[key], `signature for ${key}`)
      assert.equal(
        m.platforms[key].url,
        `https://github.com/ziee-ai/ziee/releases/download/v0.2.0/${files[key]}`,
        `url for ${key}`,
      )
    }
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('buildLatestJson defaults pub_date to an ISO timestamp', () => {
  const { dir } = makeFixtureDir()
  try {
    const m = buildLatestJson({ artifactsDir: dir, baseUrl: 'https://x/y', version: '1.0.0' })
    assert.match(m.pub_date, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/)
    assert.equal(m.notes, '')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('buildLatestJson throws on an empty directory', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'ziee-updater-empty-'))
  try {
    assert.throws(
      () => buildLatestJson({ artifactsDir: dir, baseUrl: 'https://x', version: '1.0.0' }),
      /no signed artifacts/,
    )
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('buildLatestJson throws on duplicate platform artifacts', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'ziee-updater-dup-'))
  try {
    for (const name of ['a_darwin-aarch64.tar.gz', 'b_darwin-aarch64.tar.gz']) {
      writeFileSync(path.join(dir, name), 'x')
      writeFileSync(path.join(dir, `${name}.sig`), 'c2ln\n')
    }
    assert.throws(
      () => buildLatestJson({ artifactsDir: dir, baseUrl: 'https://x', version: '1.0.0' }),
      /duplicate artifact for platform "darwin-aarch64"/,
    )
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('buildLatestJson requires its mandatory inputs', () => {
  assert.throws(() => buildLatestJson({ baseUrl: 'x', version: '1' }), /artifactsDir is required/)
})
