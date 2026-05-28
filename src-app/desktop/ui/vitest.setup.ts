/**
 * Vitest setup — runs ONCE per test file (because `isolate: true`).
 *
 * Stubs `window.__TAURI__` so any module that checks `isTauriView`
 * sees the desktop branch. Stubs localStorage/sessionStorage (jsdom
 * provides both but doesn't reset them between tests).
 */

import { beforeEach } from 'vitest'

// Tauri "in-webview" marker. Most components branch on
// `Boolean(window.__TAURI__)`; setting it to a truthy object is the
// minimum stub the type checker accepts.
;(globalThis as unknown as { __TAURI__: object }).__TAURI__ = {}

beforeEach(() => {
  // Wipe storage between tests so Zustand `persist` doesn't carry
  // state from one test to the next.
  if (typeof localStorage !== 'undefined') localStorage.clear()
  if (typeof sessionStorage !== 'undefined') sessionStorage.clear()
})
