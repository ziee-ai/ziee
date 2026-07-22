import { Auth } from '@/modules/auth/Auth.store'

/**
 * Idle, LOW-PRIORITY closure prefetch — warm the lazy tree without ever
 * competing with critical requests.
 *
 * The lazy graph (module → lazy page → lazy store-action chunk → viewer) loads
 * one dynamic-import boundary at a time, so navigation pays a serial waterfall.
 * This warms it ahead of time: on browser idle, it computes the DYNAMIC closure
 * of the chunks that have actually executed (loaded via <script>), and fetches
 * the not-yet-loaded ones with `<link rel="prefetch">`.
 *
 * Why prefetch (not modulepreload): `prefetch` is the browser's LOWEST fetch
 * priority — it uses only spare bandwidth and always yields to higher-priority
 * work (the current page's API calls and its own chunks). So this can NEVER
 * delay a critical request (e.g. the login page's auth-providers fetch), which
 * modulepreload — a High-priority signal — did. Execution stays lazy: prefetch
 * downloads + caches the bytes, but the module runs only when the real `import()`
 * reaches it.
 *
 * Gates: the `VITE_CLOSURE_PREFETCH=off` compile-time flag disables it entirely
 * (const-folded out; separate from `VITE_STORE_PREFETCH`, which gates the older
 * High-priority route prefetch), and it only runs once the user is authenticated
 * — so the unauthenticated login path is never touched. Idempotent + throttled.
 */

type Graph = Record<string, { d: string[]; s: string[] }>

let started = false

const GRAPH_URL = 'assets/ziee-preload-graph.json'
// How many prefetch links to inject per idle callback (keeps each tick short).
const PER_TICK = 6

function baseUrl(): string {
  const b = import.meta.env.BASE_URL || '/'
  return b.endsWith('/') ? b : b + '/'
}

/** Absolute URL for a chunk fileName (e.g. `assets/foo-hash.js`). */
function urlOf(fileName: string): string {
  return baseUrl() + fileName
}

/** fileNames of chunks that have actually EXECUTED (loaded via <script>), from
 *  the Resource Timing API. Prefetched chunks (initiatorType 'link') are excluded
 *  so the closure walk seeds only from real execution, never cascading off its
 *  own prefetches. */
function executedChunks(): Set<string> {
  const out = new Set<string>()
  const base = baseUrl()
  for (const e of performance.getEntriesByType('resource')) {
    if (!e.name.endsWith('.js')) continue
    if ((e as PerformanceResourceTiming).initiatorType !== 'script') continue
    // Map URL → fileName (strip origin + base).
    let path: string
    try {
      path = new URL(e.name).pathname
    } catch {
      continue
    }
    if (path.startsWith(base)) out.add(path.slice(base.length))
  }
  return out
}

export function startIdleClosurePrefetch(): void {
  if (started) return
  // Compile-time kill switch (const-folds this whole function to a no-op call).
  if (import.meta.env.VITE_CLOSURE_PREFETCH === 'off') return
  // The graph asset is emitted only by the production build (generateBundle), so
  // there's nothing to prefetch in dev / e2e — skip entirely (avoids a 404 loop).
  if (!import.meta.env.PROD) return
  if (typeof window === 'undefined') return
  if (typeof requestIdleCallback === 'undefined') return
  started = true

  let graph: Graph | null = null
  let fetching = false
  let graphUnavailable = false
  const prefetched = new Set<string>()
  const linkHead = document.head

  const injectPrefetch = (fileName: string) => {
    if (prefetched.has(fileName)) return
    prefetched.add(fileName)
    const link = document.createElement('link')
    link.rel = 'prefetch'
    link.as = 'script'
    link.href = urlOf(fileName)
    linkHead.appendChild(link)
  }

  /** Transitive dynamic closure of the executed chunks, minus what's already
   *  loaded / prefetched. Each reached chunk contributes its dynamic children AND
   *  their static deps (which load with them). */
  const buildQueue = (): string[] => {
    if (!graph) return []
    const loaded = executedChunks()
    const want = new Set<string>()
    const stack = [...loaded]
    while (stack.length) {
      const fn = stack.pop()!
      const node = graph[fn]
      if (!node) continue
      for (const d of node.d) {
        if (loaded.has(d) || want.has(d)) continue
        want.add(d)
        for (const s of graph[d]?.s ?? []) if (!loaded.has(s)) want.add(s)
        stack.push(d)
      }
    }
    const queue: string[] = []
    for (const fn of want) if (!prefetched.has(fn) && !loaded.has(fn)) queue.push(fn)
    return queue
  }

  const tick = (deadline: IdleDeadline) => {
    // Only warm once authenticated — never touch the unauthenticated login path.
    if (!Auth.$?.isAuthenticated) {
      requestIdleCallback(tick)
      return
    }
    if (!graph) {
      if (graphUnavailable) return // fetch failed / missing → give up (no loop)
      if (!fetching) {
        fetching = true
        fetch(urlOf(GRAPH_URL))
          .then(r => (r.ok ? (r.json() as Promise<Graph>) : Promise.reject()))
          .then((g: Graph) => {
            graph = g
            fetching = false
          })
          .catch(() => {
            graphUnavailable = true
            fetching = false
          })
      }
      requestIdleCallback(tick)
      return
    }

    const queue = buildQueue()
    if (queue.length === 0) return // fully warm — stop rescheduling

    let n = 0
    while (n < queue.length && (n < PER_TICK || deadline.timeRemaining() > 4)) {
      injectPrefetch(queue[n])
      n++
    }
    requestIdleCallback(tick)
  }

  requestIdleCallback(tick)
}
