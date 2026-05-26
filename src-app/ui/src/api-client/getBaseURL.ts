/**
 * Get Base URL for API calls
 *
 * This function is overridden in desktop builds using Vite alias
 * to call Tauri backend for the dynamic port.
 */

export const getBaseUrl = (function () {
  let baseUrl: Promise<string> | undefined
  return async function () {
    baseUrl ??= Promise.resolve(window.location.origin)
    return baseUrl
  }
})()
