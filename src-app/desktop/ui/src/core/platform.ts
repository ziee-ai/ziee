/**
 * Platform Detection Utilities
 *
 * Detects the current platform (Tauri, macOS, Windows, etc.)
 */

/** Check if running inside Tauri webview */
export const isTauriView = Boolean(window.__TAURI__)

/** Check if running on macOS */
export const isMacOS = /Mac|iPhone|iPad|iPod/i.test(navigator.userAgent)

/** Check if running on Windows */
export const isWindows = /Win/i.test(navigator.userAgent)

/** Check if running on Android */
export const isAndroid = /Android/i.test(navigator.userAgent)

/** Check if running on Linux */
export const isLinux = /Linux/i.test(navigator.userAgent) && !isAndroid
