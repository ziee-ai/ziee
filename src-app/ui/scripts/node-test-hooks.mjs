import { pathToFileURL } from 'node:url'
import { existsSync, statSync } from 'node:fs'
import { dirname, resolve as presolve } from 'node:path'
import { fileURLToPath } from 'node:url'
const HERE = dirname(fileURLToPath(import.meta.url))
const SRC = presolve(HERE, '../src') + '/'
const STUBS = {
  '@/core/module-system': SRC + 'core/__test-stubs__/module-system.ts',
  '@/core/events': SRC + 'core/__test-stubs__/events.ts',
}
const isFile = p => existsSync(p) && statSync(p).isFile()
export async function resolve(spec, ctx, next) {
  if (STUBS[spec]) return { url: pathToFileURL(STUBS[spec]).href, shortCircuit: true }
  if (spec.startsWith('@/')) {
    const base = SRC + spec.slice(2)
    for (const c of [base + '.ts', base + '.tsx', base + '/index.ts', base + '/index.tsx', base]) {
      if (isFile(c)) return { url: pathToFileURL(c).href, shortCircuit: true }
    }
  }
  return next(spec, ctx)
}
