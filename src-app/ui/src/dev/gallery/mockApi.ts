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
import { SAMPLE_PDF_BASE64 } from '@/modules/file/viewers/pdf/pdf-fixture'
import { base64ToBytes, makeBinaryResponse } from './mockApi-binary'

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

// ── SSE replay (serverless chat-token stream) ────────────────────────────────
// The chat UI's live states (streaming tokens, tool-call progress, elicitation
// prompts) arrive over the per-user `GET /api/chat/stream` SSE connection, not a
// JSON endpoint. To exercise those states offline we REPLAY a recorded frame
// sequence: an SSE cassette is an ordered list of `{ event, data }` frames the
// interceptor serializes into a real `text/event-stream` body — the exact wire
// shape ChatStreamClient parses (`event:` + `data:` lines). The stream stays
// open after the last frame (like a real idle connection) so a mid-generation
// cassette leaves the UI visibly streaming.

/** One recorded SSE frame: the `event:` name + the JSON `data:` payload. */
export interface SseFrame {
  event: string
  data: unknown
}
// Endpoints answered as a replayed event-stream rather than a JSON route.
const SSE_STREAM = /\/api\/chat\/stream$/
const SSE_SUBSCRIPTION = /\/api\/chat\/stream\/subscription$/
// Raw file bytes: the PDF.js viewer fetches `/files/{id}/raw` as binary. Answer
// with the sample PDF fixture so the viewer renders a real document offline.
const FILE_RAW = /^\/api\/files\/[^/]+\/raw$/
let sseCassette: SseFrame[] = []
/** Register the frame sequence the next `/api/chat/stream` request replays. */
export function setSseCassette(frames: SseFrame[]): void {
  sseCassette = frames
}
const SSE_FRAME_GAP_MS = 350

function sseResponse(frames: SseFrame[]): Response {
  const encoder = new globalThis.TextEncoder()
  const stream = new ReadableStream<Uint8Array>({
    async start(controller) {
      // A `connected` handshake first (ChatStreamClient expects it to learn its
      // connection id), then each recorded frame with a small gap so the UI
      // paints the deltas progressively.
      controller.enqueue(
        encoder.encode(
          `event: connected\ndata: ${JSON.stringify({ connectionId: 'gallery-sse' })}\n\n`,
        ),
      )
      for (const f of frames) {
        await new Promise(r => setTimeout(r, SSE_FRAME_GAP_MS))
        controller.enqueue(
          encoder.encode(`event: ${f.event}\ndata: ${JSON.stringify(f.data)}\n\n`),
        )
      }
      // Leave the stream OPEN (never close) — a real idle SSE connection. The
      // gallery frame unmounts / navigates away to tear it down.
    },
  })
  return new Response(stream, {
    status: 200,
    headers: {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
    },
  })
}

/**
 * Data-state mode. The gallery renders the SAME surface under different modes to
 * cover the states where most bugs hide (empty / error), by transforming the
 * loaded cassette response rather than maintaining parallel fixtures:
 *   - loaded : the recorded response, unchanged;
 *   - empty  : every array in the response deep-emptied + counts zeroed → the
 *              "no data yet" state;
 *   - error  : a 500 for data endpoints (auth/bootstrap exempt so the page still
 *              mounts authenticated and shows its error UI);
 *   - delayed: the loaded response after a latency, to catch the loading state.
 */
export type MockMode = 'loaded' | 'empty' | 'error' | 'delayed'
let activeMode: MockMode = 'loaded'
export function setMockMode(mode: MockMode): void {
  activeMode = mode
}
export function getMockMode(): MockMode {
  return activeMode
}

// Endpoints that must keep working even in `error` mode so the page can mount as
// an authenticated admin and render its OWN error state (not a login redirect).
const ERROR_MODE_EXEMPT = [/\/auth\/me$/, /\/setup\/status$/, /\/health$/]

const DELAY_MS = 700

/** Deep-empty arrays + zero obvious counts, preserving object shape. */
function toEmpty(value: unknown): unknown {
  if (Array.isArray(value)) return []
  if (value && typeof value === 'object') {
    const out: Record<string, unknown> = {}
    for (const [k, v] of Object.entries(value)) {
      if (Array.isArray(v)) out[k] = []
      else if (typeof v === 'number' && /(total|count|pages|unread)/i.test(k)) out[k] = 0
      else out[k] = toEmpty(v)
    }
    return out
  }
  return value
}

const jsonResponse = (data: unknown, status = 200): Response =>
  new Response(JSON.stringify(data ?? null), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })

// Crash-safe default for an UNRECORDED endpoint. An unseeded store must not
// crash the page it feeds — but no single literal shape fits every consumer
// (some read `res.items.map`, others `res.map`, others `res.total`). So the
// default is a recursive, array-like proxy:
//   - iteration / array methods (`map`/`filter`/`forEach`/…) act as an empty
//     array → `res.map(...)` and `res.items.map(...)` both yield `[]`;
//   - `length` is 0; numeric indexing is undefined;
//   - ANY other property access returns the same proxy → `res.a.b.c.map(...)`
//     is safe to arbitrary depth;
//   - it serializes to `[]` and is falsy-ish for `?? []` guards via iteration.
// The tradeoff (a scalar field read as this proxy renders oddly) only affects
// genuinely unrecorded endpoints; every gallery page records the data it needs.
const ARRAY_METHODS = new Set([
  'map', 'filter', 'forEach', 'reduce', 'reduceRight', 'find', 'findIndex',
  'some', 'every', 'slice', 'concat', 'flat', 'flatMap', 'includes',
  'indexOf', 'join', 'keys', 'values', 'entries', 'at', 'sort', 'reverse',
])

function makeSafeEmpty(): any {
  const target: any = []
  return new Proxy(target, {
    get(t, prop) {
      if (prop === Symbol.iterator) return [][Symbol.iterator].bind([])
      if (prop === 'length') return 0
      if (prop === 'toJSON') return () => []
      if (prop === Symbol.toPrimitive) return () => ''
      if (typeof prop === 'symbol') return (t as any)[prop]
      if (prop === 'then') return undefined // never a thenable
      if (ARRAY_METHODS.has(prop as string)) {
        return (...args: any[]) => (t as any)[prop](...args)
      }
      // Unknown property → recurse so deep access stays safe.
      return makeSafeEmpty()
    },
  })
}

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

    // SSE chat-token stream: replay the recorded frame cassette as a real
    // text/event-stream (NOT a JSON route). The subscription PUT is a no-op 200.
    if (SSE_SUBSCRIPTION.test(parsed.pathname)) {
      return jsonResponse({})
    }
    if (SSE_STREAM.test(parsed.pathname) && method === 'GET') {
      return sseResponse(sseCassette)
    }

    // Binary raw-file bytes (PDF viewer). Not a JSON route — serve the fixture
    // PDF so the offline gallery renders a real document. `error` mode still
    // yields a 500 so the viewer's error state is reachable in the gallery.
    if (FILE_RAW.test(parsed.pathname) && method === 'GET') {
      if (activeMode === 'error') {
        return jsonResponse(
          { error: 'Internal server error', error_code: 'GALLERY_ERROR' },
          500,
        )
      }
      return makeBinaryResponse(base64ToBytes(SAMPLE_PDF_BASE64), 'application/pdf')
    }

    // Apply the data-state mode. GET reads carry the state; mutations (POST/PUT/
    // DELETE) pass through so overlay forms can still "submit" against loaded data.
    const isRead = method === 'GET'
    const exempt = ERROR_MODE_EXEMPT.some(rx => rx.test(parsed.pathname))
    if (isRead && !exempt) {
      if (activeMode === 'error') {
        // A realistic backend-shaped 500 body (NOT a "Gallery error state"
        // dev placeholder). Well-behaved surfaces render a human ErrorState
        // and only ever expose this string behind a "Details" disclosure, so
        // when it does surface it reads like a genuine server error. The
        // `error_code` keeps the gallery tooling's error-mode detection.
        return jsonResponse(
          { error: 'Internal server error', error_code: 'GALLERY_ERROR' },
          500,
        )
      }
      if (activeMode === 'delayed') {
        await new Promise(r => setTimeout(r, DELAY_MS))
      }
    }

    const matched = matchRoute(method, parsed.pathname)
    if (!matched) {
      if (import.meta.env.DEV) {
        console.warn(`[gallery mockApi] no route for ${method} ${parsed.pathname}`)
      }
      return jsonResponse(makeSafeEmpty())
    }

    const entry = activeCassette[matched.route.key]
    if (entry === undefined) {
      if (import.meta.env.DEV) {
        console.warn(
          `[gallery mockApi] no cassette for ${matched.route.key} (${method} ${parsed.pathname})`,
        )
      }
      return jsonResponse(makeSafeEmpty())
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
    let value =
      typeof entry === 'function'
        ? (entry as (c: MockRequestContext) => unknown)(ctx)
        : entry
    // `empty` mode: return a valid, well-shaped empty response (arrays emptied,
    // counts zeroed) — the state where "no data yet" bugs live.
    if (isRead && !exempt && activeMode === 'empty') {
      value = toEmpty(value)
    }
    return jsonResponse(value)
  }
}

/** Restore the original fetch (used by tests / teardown). */
export function uninstallMockApi(): void {
  if (originalFetch) globalThis.fetch = originalFetch
  installed = false
}
