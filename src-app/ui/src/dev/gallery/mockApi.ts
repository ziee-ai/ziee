/**
 * Mock-API layer for the seeded gallery.
 *
 * Installs a `window.fetch` interceptor that answers `/api/*` requests from
 * per-module SEED CASSETTES instead of a live backend. Every store's REAL
 * `load()` path runs unchanged (same api-client → same fetch), so pages and
 * components populate exactly as in production — just deterministically and
 * offline. Dev-only; installed by the gallery bootstrap (`seed.ts`).
 *
 * Correctness (see SEEDED_GALLERY_PLAN.md):
 *  1. cassette values are typed against the generated api-client response types
 *     (`GetResponseType<K>`) — a wrong shape fails `tsc`;
 *  2. cassettes are RECORDED from a real server (`scripts/record-gallery-fixtures.mjs`);
 *  3. a contract test validates each cassette against `openapi.json`.
 */
import {
  ApiEndpoints,
  type ApiEndpoint,
  type GetResponseType,
} from '@/api-client/types'

/** Context handed to a cassette resolver function for one request. */
export interface MockRequestContext {
  /** Path captures, e.g. `{ provider_id: '…' }`. */
  params: Record<string, string>
  /** Parsed query string, e.g. `{ providerId: '…', page: '1' }`. */
  query: Record<string, string>
  /** Parsed JSON request body (mutations); `undefined` for GET/empty. */
  body: unknown
  method: string
}

/**
 * One cassette entry: either a literal recorded response, or a resolver that
 * derives it from the request (e.g. `LlmModel.list` keyed by `?providerId=`).
 * Typed by endpoint so a shape mismatch fails the build.
 */
export type CassetteEntry<K extends ApiEndpoint> =
  | GetResponseType<K>
  | ((ctx: MockRequestContext) => GetResponseType<K>)

/** A partial map from endpoint key → recorded/derived response. */
export type Cassette = {
  [K in ApiEndpoint]?: CassetteEntry<K>
}

interface CompiledRoute {
  key: ApiEndpoint
  method: string
  regex: RegExp
  paramNames: string[]
}

// Precompile every endpoint URL pattern (`METHOD /api/x/{cap}`) into a matcher
// so a concrete request path resolves back to its endpoint key.
const COMPILED: CompiledRoute[] = Object.entries(ApiEndpoints).map(
  ([key, url]) => {
    const [method, pattern] = (url as string).split(' ') as [string, string]
    const paramNames: string[] = []
    const source = pattern
      .replace(/[.*+?^${}()|[\]\\]/g, m => `\\${m}`) // escape regex metachars
      .replace(/\\\{([^}]+)\\\}/g, (_all, name: string) => {
        paramNames.push(name)
        return '([^/]+)'
      })
    return {
      key: key as ApiEndpoint,
      method,
      regex: new RegExp(`^${source}$`),
      paramNames,
    }
  },
)

function matchRoute(
  method: string,
  path: string,
): { route: CompiledRoute; params: Record<string, string> } | undefined {
  // Prefer the most specific match: a literal segment count tie-breaker keeps
  // `/api/llm-providers/{id}` from shadowing `/api/llm-providers`.
  let best: { route: CompiledRoute; params: Record<string, string> } | undefined
  for (const route of COMPILED) {
    if (route.method !== method) continue
    const m = route.regex.exec(path)
    if (!m) continue
    const params: Record<string, string> = {}
    route.paramNames.forEach((name, i) => {
      params[name] = decodeURIComponent(m[i + 1])
    })
    // Fewer captures = more literal = more specific → keep the tightest.
    if (!best || route.paramNames.length < best.route.paramNames.length) {
      best = { route, params }
    }
  }
  return best
}

let activeCassette: Cassette = {}
let installed = false
let originalFetch: typeof globalThis.fetch | undefined

const jsonResponse = (data: unknown, status = 200): Response =>
  new Response(JSON.stringify(data ?? null), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })

// Permissive default for an unrecorded endpoint: a superset shape whose common
// list/pagination accessors resolve to empties, so an unseeded store degrades to
// "empty" instead of throwing on `res.items`/`res.providers`/….
const EMPTY_DEFAULT = {
  page: 1,
  per_page: 50,
  total: 0,
  providers: [],
  models: [],
  items: [],
  data: [],
  results: [],
  entries: [],
  conversations: [],
  servers: [],
  files: [],
} as const

/** Register (replace) the cassette the interceptor answers from. */
export function setCassette(cassette: Cassette): void {
  activeCassette = cassette
}

/** Merge additional entries into the active cassette. */
export function extendCassette(cassette: Cassette): void {
  activeCassette = { ...activeCassette, ...cassette }
}

/**
 * Install the `window.fetch` interceptor (idempotent). Non-`/api` requests and
 * SSE/stream endpoints pass through to the real fetch (vite assets, HMR).
 */
export function installMockApi(cassette?: Cassette): void {
  if (cassette) setCassette(cassette)
  if (installed) return
  installed = true
  originalFetch = globalThis.fetch.bind(globalThis)

  globalThis.fetch = async (
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> => {
    const url =
      typeof input === 'string'
        ? input
        : input instanceof URL
          ? input.href
          : input.url
    const method = (
      init?.method ??
      (input instanceof Request ? input.method : 'GET')
    ).toUpperCase()

    let parsed: URL
    try {
      parsed = new URL(url, window.location.origin)
    } catch {
      return originalFetch!(input as RequestInfo, init)
    }

    // Only intercept same-origin API calls; everything else is real.
    if (parsed.origin !== window.location.origin || !parsed.pathname.startsWith('/api/')) {
      return originalFetch!(input as RequestInfo, init)
    }

    const matched = matchRoute(method, parsed.pathname)
    if (!matched) {
      if (import.meta.env.DEV) {
        console.warn(`[gallery mockApi] no route for ${method} ${parsed.pathname}`)
      }
      return jsonResponse(EMPTY_DEFAULT)
    }

    const entry = activeCassette[matched.route.key]
    if (entry === undefined) {
      if (import.meta.env.DEV) {
        console.warn(
          `[gallery mockApi] no cassette for ${matched.route.key} (${method} ${parsed.pathname})`,
        )
      }
      return jsonResponse(EMPTY_DEFAULT)
    }

    const query: Record<string, string> = {}
    parsed.searchParams.forEach((v, k) => {
      query[k] = v
    })

    let body: unknown
    if (init?.body && typeof init.body === 'string') {
      try {
        body = JSON.parse(init.body)
      } catch {
        body = init.body
      }
    }

    const ctx: MockRequestContext = {
      params: matched.params,
      query,
      body,
      method,
    }
    const value =
      typeof entry === 'function'
        ? (entry as (c: MockRequestContext) => unknown)(ctx)
        : entry
    return jsonResponse(value)
  }
}

/** Restore the original fetch (used by tests / teardown). */
export function uninstallMockApi(): void {
  if (originalFetch) globalThis.fetch = originalFetch
  installed = false
}
