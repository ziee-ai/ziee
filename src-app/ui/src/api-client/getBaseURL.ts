/**
 * Get Base URL for API calls
 *
 * This function is overridden in desktop builds using Vite alias
 * to call Tauri backend for the dynamic port.
 */

export const getBaseUrl = (function () {
  let baseUrl: Promise<string>
  //@ts-ignore
  return async function () {
    if (baseUrl) {
      return baseUrl // Return existing promise if already created
    }
    return window.location.origin
  }
})()
