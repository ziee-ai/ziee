#!/usr/bin/env node
// Tauri WebDriver smoke (Layer 3).
//
// Spawns tauri-driver, boots the production-bundled Ziee.app via
// W3C WebDriver, asserts a bare minimum: window opens, React root
// renders, title contains "Ziee". Exits 0/1.
//
// See ./README.md for prerequisites.

import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { Builder, By, until } from 'selenium-webdriver'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

// desktop/ui/tests/tauri-driver -> src-app
const SRC_APP = resolve(__dirname, '../../../..')
const BUNDLE_PATH = resolve(SRC_APP, 'target/release/bundle/macos/Ziee.app')
const TAURI_DRIVER_PORT = 4444
const BRINGUP_TIMEOUT_MS = 60_000
const TITLE_MATCH = /ziee/i

function fail(msg) {
  console.error(`[tauri-driver-smoke] FAIL: ${msg}`)
  process.exit(1)
}

if (!existsSync(BUNDLE_PATH)) {
  fail(
    `bundle not built at ${BUNDLE_PATH} — run \`cd desktop/tauri && cargo tauri build\` first`,
  )
}

console.log(`[tauri-driver-smoke] bundle = ${BUNDLE_PATH}`)
console.log(`[tauri-driver-smoke] port   = ${TAURI_DRIVER_PORT}`)

// Spawn tauri-driver in the background. It listens on
// TAURI_DRIVER_PORT and forwards W3C WebDriver commands to the
// platform driver (safaridriver on macOS).
const driver = spawn('tauri-driver', ['--port', String(TAURI_DRIVER_PORT)], {
  stdio: ['ignore', 'inherit', 'inherit'],
})

let exited = false
driver.on('exit', (code) => {
  exited = true
  if (code !== 0 && code !== null) {
    console.error(`[tauri-driver-smoke] tauri-driver exited with code ${code}`)
  }
})

// Cleanup on any exit path.
const cleanup = () => {
  if (!exited) {
    try {
      driver.kill('SIGTERM')
    } catch {}
  }
}
process.on('exit', cleanup)
process.on('SIGINT', () => {
  cleanup()
  process.exit(130)
})

// Give tauri-driver a moment to bind its port. 1s is plenty on a
// dev box; the cost of being wrong here is a clearer error from
// the Builder.connect path below.
await new Promise((r) => setTimeout(r, 1_000))

let session
try {
  session = await new Builder()
    .usingServer(`http://127.0.0.1:${TAURI_DRIVER_PORT}`)
    .withCapabilities({
      'tauri:options': { application: BUNDLE_PATH },
      // Tauri's webview reports as "wry" but the W3C spec wants a
      // browserName. tauri-driver maps anything sane to its inner
      // driver; "wry" is the conventional value.
      browserName: 'wry',
    })
    .build()

  // Wait for the SPA's React root. The desktop SPA mounts under
  // <div id="root">…</div>; the auth flow may delay first paint
  // while auto-login runs.
  await session.wait(until.elementLocated(By.css('#root *')), BRINGUP_TIMEOUT_MS)

  const title = await session.getTitle()
  console.log(`[tauri-driver-smoke] window title = "${title}"`)

  if (!TITLE_MATCH.test(title)) {
    throw new Error(
      `window title "${title}" doesn't match ${TITLE_MATCH} — bundling regression?`,
    )
  }

  console.log('[tauri-driver-smoke] PASS')
  process.exit(0)
} catch (err) {
  console.error('[tauri-driver-smoke] error:', err?.message || err)
  process.exit(1)
} finally {
  if (session) {
    try {
      await session.quit()
    } catch {}
  }
}
