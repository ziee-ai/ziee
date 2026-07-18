import { useEffect, useState } from 'react'
import { Stores } from '@ziee/framework/stores'

/**
 * URL-keyed cache for fetched resource_link text contents.
 *
 * Lives at module scope so it survives component unmount/remount
 * (collapse-then-expand, scrolling a long message off and back on
 * screen, switching conversations and returning). Mirrors the
 * dedup behaviour of `Stores.File.fileTextContents` but
 * keyed by URL rather than UUID, because chat-inline `resource_link`
 * blocks have no FileEntity.
 *
 * The cache is intentionally unbounded for now — a single message
 * typically references a handful of files at most, and the entries
 * are small strings. If a future workload pushes the cache pathological,
 * add an LRU here.
 */
const textCache = new Map<string, string>()
const inFlight = new Map<string, Promise<string>>()
const errorCache = new Map<string, string>()

const ERROR_SENTINEL = '__error__'

export type ResourceLinkContent = string | null | '__error__'

/**
 * Fetch a resource_link's body as text, with module-level deduplication.
 *
 * Returns:
 *  - `null` while the initial fetch is in flight (caller renders a
 *    `<Spin>`).
 *  - the fetched string when ready.
 *  - `'__error__'` when the fetch failed (caller renders an inline
 *    error message instead of an infinite spinner).
 *
 * Guards: only fetches `/api/...` URLs. Absolute URLs to external
 * hosts are refused — those should never appear from a trusted MCP
 * source, and a `text/*` viewer reaching out to `https://evil.com/`
 * would be both a data-exfil and a content-spoofing vector. The
 * inline-render dispatcher also rejects non-`/api/` URIs upstream,
 * so this is belt-and-suspenders.
 */
export function useResourceLinkContent(
  url: string,
  skip = false,
): ResourceLinkContent {
  const cached = !skip && url ? textCache.get(url) ?? null : null
  const errored = !skip && url ? errorCache.has(url) : false
  const [, force] = useState(0)

  useEffect(() => {
    if (skip || !url) return
    // Re-read the cache LIVE inside the effect (not via the closured
    // `cached`/`errored` from render time). This handles the race
    // where another consumer's fetch resolves between our render and
    // our effect — without it we'd start a needless second fetch.
    if (textCache.has(url)) {
      if (cached === null) force(n => n + 1)
      return
    }
    if (errorCache.has(url)) {
      if (!errored) force(n => n + 1)
      return
    }
    if (!isLocalApiUrl(url)) {
      errorCache.set(url, 'Refused to fetch external URL')
      force(n => n + 1)
      return
    }
    let cancelled = false
    let promise = inFlight.get(url)
    if (!promise) {
      // Delegate the actual fetch to the file extension store so the
      // request carries the bearer token (the previous inline `fetch`
      // was unauthenticated). The hook keeps the module-scope text +
      // error caches because they outlive both the store __destroy__
      // and individual component unmounts in a way that matches user
      // expectations (collapse-then-expand should not re-fetch).
      promise = Stores.File.fetchResourceLinkText(url)
        .then(text => {
          textCache.set(url, text)
          inFlight.delete(url)
          return text
        })
        .catch(err => {
          errorCache.set(url, String(err?.message ?? err))
          inFlight.delete(url)
          throw err
        })
      inFlight.set(url, promise)
    }
    promise
      .then(() => {
        if (!cancelled) force(n => n + 1)
      })
      .catch(() => {
        if (!cancelled) force(n => n + 1)
      })
    return () => {
      cancelled = true
    }
    // Intentionally NOT depending on `cached` / `errored`: those are
    // closured snapshots; the effect reads the live caches above.
    // Re-running on every cache flip would just cause extra no-op
    // effect runs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [url, skip])

  if (skip || !url) return null
  if (errored) return ERROR_SENTINEL
  return cached
}

/**
 * Read the error message recorded for a URL by a previous
 * `useResourceLinkContent` call. Only meaningful after the hook
 * returned `'__error__'` — viewers use this to show a specific
 * message rather than a generic "failed to load".
 */
export function getResourceLinkError(url: string): string | undefined {
  return errorCache.get(url)
}

/**
 * Drop any cached content / error for a URL. Useful for retry buttons.
 * The corresponding `useResourceLinkContent` consumer needs to be
 * re-rendered for the refetch to actually fire.
 */
export function invalidateResourceLink(url: string): void {
  textCache.delete(url)
  errorCache.delete(url)
}

function isLocalApiUrl(url: string): boolean {
  // Accept relative `/api/...` paths.
  if (url.startsWith('/api/')) return true
  // Accept same-origin absolute URLs that happen to include the
  // origin (e.g. `http://localhost:9000/api/...`). The download
  // handler's `loopback_url` flows through here.
  try {
    const u = new URL(url, window.location.origin)
    if (u.origin !== window.location.origin) return false
    return u.pathname.startsWith('/api/')
  } catch {
    return false
  }
}
