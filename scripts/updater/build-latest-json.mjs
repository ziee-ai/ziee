#!/usr/bin/env node
// Build a Tauri static updater manifest (`latest.json`) from a directory of
// signed bundles.
//
// This is the SINGLE source of truth for manifest assembly: both the
// production release workflow (.github/workflows/desktop-release.yml) and the
// local CI test (.github/workflows/desktop-updater-pages-test.yml) call it, so
// testing this module ≈ testing the job.
//
// Input: a flat directory containing, per platform, an updater artifact whose
// filename embeds the Tauri platform key, plus a sibling `<artifact>.sig`
// (base64, exactly as `tauri signer sign` / tauri-action emit). Example:
//   ziee_0.2.0_darwin-aarch64.app.tar.gz
//   ziee_0.2.0_darwin-aarch64.app.tar.gz.sig
//
// Output: Tauri static-manifest JSON
//   { version, notes, pub_date, platforms: { "<key>": { signature, url } } }
// where url = <base-url>/<artifact-filename> (the GitHub Release asset URL the
// updater downloads from; the signature is embedded here for verification).

import { readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

/** The four Tauri updater platform keys we publish. */
export const PLATFORM_KEYS = [
  'darwin-aarch64',
  'darwin-x86_64',
  'linux-x86_64',
  'windows-x86_64',
]

/** Find the platform key embedded in an artifact filename, or null. */
export function platformKeyForFilename(filename) {
  // Longest-first so e.g. `darwin-x86_64` is preferred over a hypothetical
  // shorter overlap. The keys are mutually unambiguous as full substrings.
  for (const key of [...PLATFORM_KEYS].sort((a, b) => b.length - a.length)) {
    if (filename.includes(key)) return key
  }
  return null
}

function joinUrl(baseUrl, name) {
  return `${baseUrl.replace(/\/+$/, '')}/${name}`
}

/**
 * Assemble the manifest object from a directory of signed artifacts.
 * @returns {{version:string, notes:string, pub_date:string, platforms:Object}}
 */
export function buildLatestJson({ artifactsDir, baseUrl, version, notes = '', pubDate }) {
  if (!artifactsDir) throw new Error('artifactsDir is required')
  if (!baseUrl) throw new Error('baseUrl is required')
  if (!version) throw new Error('version is required')

  const entries = readdirSync(artifactsDir)
  // Drive off the `.sig` files: each signed updater artifact has exactly one.
  const sigFiles = entries.filter((f) => f.endsWith('.sig'))

  const platforms = {}
  for (const sigFile of sigFiles.sort()) {
    const artifactName = sigFile.slice(0, -'.sig'.length)
    const key = platformKeyForFilename(artifactName)
    if (!key) {
      console.warn(`[build-latest-json] skip: no platform key in "${artifactName}"`)
      continue
    }
    if (platforms[key]) {
      throw new Error(
        `duplicate artifact for platform "${key}": already had ${platforms[key].url}, also found ${artifactName}`,
      )
    }
    const signature = readFileSync(path.join(artifactsDir, sigFile), 'utf8').trim()
    if (!signature) throw new Error(`empty signature file: ${sigFile}`)
    platforms[key] = { signature, url: joinUrl(baseUrl, artifactName) }
  }

  if (Object.keys(platforms).length === 0) {
    throw new Error(`no signed artifacts (*.sig) found in ${artifactsDir}`)
  }

  return {
    version,
    notes,
    pub_date: pubDate ?? new Date().toISOString(),
    platforms,
  }
}

// ---- CLI -------------------------------------------------------------------

function parseArgs(argv) {
  const args = {}
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a.startsWith('--')) {
      const key = a.slice(2)
      const val = argv[i + 1]?.startsWith('--') ? 'true' : argv[++i]
      args[key] = val
    }
  }
  return args
}

function main() {
  const a = parseArgs(process.argv.slice(2))
  const manifest = buildLatestJson({
    artifactsDir: a['artifacts-dir'],
    baseUrl: a['base-url'],
    version: a['version'],
    notes: a['notes'] ?? '',
    pubDate: a['pub-date'],
  })
  const json = JSON.stringify(manifest, null, 2)
  if (a['out'] && a['out'] !== '-') {
    writeFileSync(a['out'], json + '\n')
    console.error(`[build-latest-json] wrote ${a['out']} (${Object.keys(manifest.platforms).length} platforms)`)
  } else {
    process.stdout.write(json + '\n')
  }
}

// Run main() only when invoked directly, not when imported by the test.
if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main()
}
