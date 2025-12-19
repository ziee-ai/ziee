/**
 * Desktop Override for getBaseURL
 *
 * This version calls the Tauri backend to get the dynamic server port
 * instead of using window.location.origin
 */

import { invoke } from '@tauri-apps/api/core'

export const getBaseUrl = (function () {
  let baseUrl: Promise<string>

  return async function () {
    if (baseUrl) {
      return baseUrl // Return existing promise if already created
    }

    // Call Tauri backend to get server port
    baseUrl = invoke<number>('get_server_port')
      .then(port => {
        const url = `http://127.0.0.1:${port}`
        console.log(`[Desktop] API Base URL: ${url}`)
        return url
      })
      .catch(error => {
        console.error('[Desktop] Failed to get server port:', error)
        // Fallback to default port if backend not available
        return window.location.origin
      })

    return baseUrl
  }
})()
