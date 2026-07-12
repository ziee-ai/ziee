/**
 * Desktop Override for getBaseURL
 *
 * Calls Tauri backend to get the dynamic server port
 * instead of using window.location.origin
 */

import { invoke } from '@tauri-apps/api/core'

export const getBaseUrl = (function () {
  let baseUrl: Promise<string>

  return async function () {
    if (baseUrl) {
      return baseUrl // Return existing promise if already created
    }

    // Phone / non-Tauri browser path: the bundle is served over the
    // ngrok tunnel and the same-origin URL IS the API root. Short-
    // circuit BEFORE calling invoke() — without this, some
    // @tauri-apps/api versions hang instead of rejecting, leaving
    // every API call awaiting forever.
    if (typeof window === 'undefined' || !(window as any).__TAURI__) {
      baseUrl = Promise.resolve(window.location.origin)
      return baseUrl
    }

    // Call Tauri backend to get server port
    baseUrl = invoke<number>('get_server_port')
      .then(port => {
        const url = `http://127.0.0.1:${port}`
        console.log(`[Desktop] API Base URL: ${url}`)
        return url
      })
      .catch(() => {
        // Fallback to default port if backend not available
        return window.location.origin
      })

    return baseUrl
  }
})()
