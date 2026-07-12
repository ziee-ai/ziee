import type { ComponentType } from 'react'
import { isTauriView } from '@ziee/desktop/core/platform'

/**
 * Desktop override of core's `lazyWithPreload`.
 *
 * Resolved by Vite's `localOverridePlugin`: when desktop UI code
 * imports `@/utils/lazyWithPreload`, this file is served INSTEAD
 * of `src-app/ui/src/utils/lazyWithPreload.ts`.
 *
 * Two behaviors selected at module-load:
 *
 *   - **Inside the Tauri webview** (`isTauriView === true`):
 *     `factory()` is called IMMEDIATELY at registration time. Every
 *     page chunk starts fetching during module load; by the time
 *     the user clicks the first route, the import is already done
 *     (or nearly done) and `Suspense` resolves without showing a
 *     fallback. Desktop ships every chunk inside the binary, so the
 *     webview pays no network cost for this — and route
 *     transitions in a native-feeling app should be instantaneous.
 *
 *   - **NOT inside the Tauri webview** (remote browser via the
 *     Remote Access ngrok tunnel, or any other proxy): fall through
 *     to core's deferred behavior. The remote browser is doing a
 *     real network fetch over the tunnel for every chunk; eager-
 *     loading every page on first paint would multiply the boot
 *     download by ~10x and hurt time-to-interactive on slow links.
 *     Lazy deferral is exactly what code-splitting is for there.
 *
 * Why not skip `Suspense` entirely on the Tauri path: this override
 * preserves the function signature core defines, so
 * `LazyComponentRenderer`'s `isLikelyLazy` heuristic still
 * classifies the result as lazy and routes it through `React.lazy()`.
 * Pre-resolving the promise is enough: `React.lazy` checks the
 * promise state and skips the fallback when already-resolved.
 */
export function lazyWithPreload<T extends ComponentType<any>>(
  factory: () => Promise<{ default: T }>,
): () => Promise<{ default: T }> {
  if (isTauriView) {
    // Tauri webview — kick off the import the moment registration
    // calls `lazyWithPreload(() => import('./SomePage'))`. Same
    // single-promise contract as core, just resolved sooner.
    const promise = factory()
    return () => promise
  }

  // Remote-browser path (Remote Access tunnel) — mirror core's
  // deferred behavior verbatim so chunks load on first navigation,
  // not on first paint.
  let promise: Promise<{ default: T }> | null = null
  return () => {
    if (!promise) {
      promise = factory()
    }
    return promise
  }
}
