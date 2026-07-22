import { pathToFileURL } from 'node:url'
import { existsSync, realpathSync, statSync } from 'node:fs'
import { dirname, resolve as presolve } from 'node:path'
import { fileURLToPath } from 'node:url'
const HERE = dirname(fileURLToPath(import.meta.url))
const SRC = presolve(HERE, '../src') + '/'
const STUBS = {
  '@/core/module-system': SRC + 'core/__test-stubs__/module-system.ts',
  '@/core/events': SRC + 'core/__test-stubs__/events.ts',
}
const isFile = p => existsSync(p) && statSync(p).isFile()
const CANDIDATES = ['.ts', '.tsx', '/index.ts', '/index.tsx']

export async function resolve(spec, ctx, next) {
  if (STUBS[spec]) return { url: pathToFileURL(STUBS[spec]).href, shortCircuit: true }
  if (spec.startsWith('@/')) {
    const base = SRC + spec.slice(2)
    for (const c of [...CANDIDATES.map(e => base + e), base]) {
      if (isFile(c)) return { url: pathToFileURL(c).href, shortCircuit: true }
    }
  }
  try {
    return await next(spec, ctx)
  } catch (err) {
    // Extensionless subpath of a workspace package (`@ziee/framework/store-kit`
    // → exports `"./*": "./src/*"` → `…/src/store-kit`, with no extension).
    // Vite resolves that; Node's ESM resolver requires the exact filename, so
    // every spec importing a store that pulls in `store-kit` died at import time
    // with ERR_MODULE_NOT_FOUND — 10 files, none of which had anything wrong
    // with them. Retry the resolved path with the same extension candidates
    // used for `@/` above.
    // ERR_MODULE_NOT_FOUND: an extensionless file (`…/src/store-kit`).
    // ERR_UNSUPPORTED_DIR_IMPORT: a directory needing `/index.ts` (`…/src/events`).
    // Both are "Vite would have resolved this" cases; both carry the resolved
    // `url` we need to retry from.
    if (
      (err?.code !== 'ERR_MODULE_NOT_FOUND' &&
        err?.code !== 'ERR_UNSUPPORTED_DIR_IMPORT') ||
      !err.url
    ) {
      throw err
    }
    const base = fileURLToPath(err.url).replace(/\/$/, '')
    for (const c of CANDIDATES.map(e => base + e)) {
      // realpath, NOT the resolved path: a workspace package is a SYMLINK under
      // node_modules (`@ziee/framework` → `sdk/packages/framework`), and Node
      // refuses to strip types for anything under node_modules
      // (ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING). Handing back the real
      // source location makes it plain TypeScript source again.
      if (isFile(c)) return { url: pathToFileURL(realpathSync(c)).href, shortCircuit: true }
    }
    throw err
  }
}
