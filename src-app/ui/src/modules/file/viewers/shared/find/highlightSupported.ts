// Feature-detect the CSS Custom Highlight API. When it's unavailable (older
// Safari/Firefox, or a non-DOM test env) the find UI is not rendered and Ctrl-F
// is not intercepted, so the browser's native find takes over — a graceful
// fallback rather than a broken button.

export function isHighlightSupported(): boolean {
  return (
    typeof CSS !== 'undefined' &&
    // `highlights` is the registry; `Highlight` is the range-set constructor.
    !!(CSS as unknown as { highlights?: unknown }).highlights &&
    typeof (globalThis as { Highlight?: unknown }).Highlight === 'function'
  )
}
