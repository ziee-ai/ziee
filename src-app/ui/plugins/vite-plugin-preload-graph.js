/**
 * vite-plugin-preload-graph — emit the chunk dependency graph as a static asset
 * so the runtime can compute, on idle, the DYNAMIC closure of already-loaded
 * chunks and warm it with LOW-PRIORITY `<link rel="prefetch">`.
 *
 * Unlike modulepreload (High priority — competes with API/critical requests),
 * `prefetch` is the browser's Lowest priority: it only uses spare bandwidth and
 * always yields to critical requests. So this never delays the current page's
 * API calls or chunks; it just fills idle gaps so FUTURE navigation is cache-warm.
 * The graph is emitted (final hashed names known at generateBundle time); the
 * runtime (src/core/preload/idleClosurePrefetch.ts) fetches it once, on idle,
 * after auth.
 *
 * Emits `assets/ziee-preload-graph.json`:
 *   { "<chunkFileName>": { "d": [dynamicImportFileNames], "s": [staticImportFileNames] }, … }
 */
export function preloadGraphPlugin() {
  return {
    name: 'ziee-preload-graph',
    generateBundle: {
      // `post` so all chunk fileNames/import lists are final.
      order: 'post',
      handler(_options, bundle) {
        // Identify MODULE-boundary chunks (a `modules/**/module.tsx` — the units
        // the smart-loader gates by auth/permission/path). The manifest's
        // `load()` dynamically imports these, so an unfiltered closure from the
        // entry would reach EVERY module — prefetching code the user isn't
        // permitted to load (e.g. admin modules for a non-admin), defeating the
        // gating. We drop dynamic edges that POINT AT a module boundary: the
        // closure then only warms the internal lazy tree (page → viewer → action)
        // of modules that are ALREADY loaded (eligible modules are login-wave
        // seeds), never crossing into an ungated/ineligible module.
        const isModuleBoundary = fn => {
          const c = bundle[fn]
          const id = c && c.type === 'chunk' ? c.facadeModuleId : null
          return !!id && /[\\/]modules[\\/].*[\\/]module\.tsx$/.test(id)
        }
        const moduleChunks = new Set()
        for (const fileName in bundle) {
          if (isModuleBoundary(fileName)) moduleChunks.add(bundle[fileName].fileName)
        }

        const graph = {}
        for (const fileName in bundle) {
          const c = bundle[fileName]
          if (c.type !== 'chunk') continue
          // Drop dynamic edges into a module boundary (the manifest load()s).
          const d = (c.dynamicImports ?? []).filter(t => !moduleChunks.has(t))
          const s = c.imports ?? []
          if (d.length || s.length) {
            graph[c.fileName] = { d, s }
          }
        }
        this.emitFile({
          type: 'asset',
          // Fixed (un-hashed) name so the runtime can fetch a stable URL.
          fileName: 'assets/ziee-preload-graph.json',
          source: JSON.stringify(graph),
        })
      },
    },
  }
}
