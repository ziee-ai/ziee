/**
 * vite-plugin-parallel-preload — flatten the lazy-import waterfall.
 *
 * Vite already emits `<link rel="modulepreload">` for a dynamically-imported
 * chunk's STATIC deps, so those load in parallel. But NESTED dynamic imports
 * (module → lazy page → lazy store-action chunk → …) are each their own boundary
 * Vite won't cross, so they resolve one-after-another — a serial request
 * waterfall (a "straight line" in the network panel).
 *
 * This plugin extends each dynamic import's preload list to its ENTIRE downstream
 * DYNAMIC closure. When a chunk is dynamically imported, its whole lazy subtree
 * is `modulepreload`ed at once → fetched in PARALLEL. Execution stays lazy:
 * `modulepreload` downloads + compiles the module but does NOT run its code until
 * the real `import()` reaches it. So nothing becomes eager — only the network
 * fetch is pulled forward. By the time the code `import()`s the page / the store
 * action, its bytes are already cached → the N-deep waterfall collapses to ~1
 * round-trip of depth.
 *
 * Scoping: Vite bakes the preload list into each `import()` site's `__vitePreload`
 * call, so a module's closure is only fetched when that module is actually
 * imported (here: when the smart-loader gates it in). We never preload the whole
 * app up front — only the feature subtree being loaded, just in parallel.
 *
 * Usage:
 *   const { plugin, resolveDependencies } = parallelPreloadPlugin()
 *   plugins: [ ..., plugin ]
 *   build: { modulePreload: { resolveDependencies } }
 */
export function parallelPreloadPlugin(opts = {}) {
  // Safety cap so a single import can't preload an unbounded tree. Depth is in
  // dynamic-import hops; the app's deepest chain (module → page → list → viewer →
  // action) is ~5, so 8 is generous headroom.
  const maxDepth = opts.maxDepth ?? 8

  // chunk fileName -> { imports: string[], dynamicImports: string[] } — built in
  // generateBundle from the FINAL bundle (renderChunk fileNames still carry hash
  // placeholders, so they wouldn't match resolveDependencies' final names).
  const graph = new Map()
  const closureCache = new Map()

  /** Transitive DYNAMIC closure of `fileName`: every chunk reachable by following
   *  dynamicImports (bounded by maxDepth), plus each such chunk's STATIC imports
   *  (they load with it). Excludes `fileName` itself and its own static deps —
   *  Vite already put those in `deps`. */
  function closureOf(fileName) {
    if (closureCache.has(fileName)) return closureCache.get(fileName)
    const out = new Set()
    const visit = (fn, depth) => {
      if (depth > maxDepth) return
      const node = graph.get(fn)
      if (!node) return
      for (const d of node.dynamicImports) {
        const firstSee = !out.has(d)
        out.add(d)
        for (const s of graph.get(d)?.imports ?? []) out.add(s)
        if (firstSee) visit(d, depth + 1)
      }
    }
    visit(fileName, 0)
    out.delete(fileName)
    closureCache.set(fileName, out)
    return out
  }

  const plugin = {
    name: 'ziee-parallel-preload',
    generateBundle: {
      // `pre` so the graph is built before Vite's import-analysis generateBundle
      // (which calls resolveDependencies) runs.
      order: 'pre',
      handler(_options, bundle) {
        graph.clear()
        closureCache.clear()
        for (const fileName in bundle) {
          const c = bundle[fileName]
          if (c.type === 'chunk') {
            graph.set(c.fileName, {
              imports: c.imports ?? [],
              dynamicImports: c.dynamicImports ?? [],
            })
          }
        }
      },
    },
  }

  /** Vite `build.modulePreload.resolveDependencies` hook: `deps` is the chunk's
   *  static-dep closure (Vite's default). Add the dynamic closure so the whole
   *  lazy subtree preloads in parallel — but ONLY for runtime `js` dynamic
   *  imports (the actual lazy boundaries). For `html` (the entry's first-paint
   *  preloads) we leave `deps` untouched: augmenting there would preload the
   *  entry's ENTIRE dynamic closure — the whole app — on initial load, defeating
   *  code-splitting. Each module's closure is instead fetched only when that
   *  module is dynamically imported (i.e. when the smart-loader gates it in). */
  const resolveDependencies = (filename, deps, ctx) => {
    if (ctx && ctx.hostType === 'html') return deps
    const extra = closureOf(filename)
    if (extra.size === 0) return deps
    return Array.from(new Set([...deps, ...extra]))
  }

  return { plugin, resolveDependencies }
}
