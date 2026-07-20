/**
 * Vite plugin: inline generated map lookups (dev + prod).
 *
 * Rewrites, at build time (after tsc, so types are untouched):
 *   - `Permissions.<Member>`     → the permission string literal ("users::read")
 *   - `ApiClient.<NS>.<method>(`  → `callAsync("METHOD /url", `   [enabled once
 *                                    apiEndpoints.ts is split out — see `endpoints`]
 *
 * Why: these generated maps (`Permissions`, `ApiEndpoints`) otherwise ship whole
 * in the eager entry chunk and grow unbounded with the API/permission surface.
 * Inlining makes them CALL-SITE GRANULAR — each chunk carries only the literals
 * its own code uses — so the entry chunk stays flat as the app scales. A symbol's
 * now-unused import is stripped so the map object itself tree-shakes to where it's
 * still enumerated (e.g. the lazy permission picker's `Object.entries(Permissions)`).
 *
 * Pure text edits via magic-string — no TS parse. Safe: the codebase has no
 * dynamic (`Permissions[x]` / `ApiClient[x]`) uses of these as call targets.
 * Test/spec files are skipped so mocks keep working.
 */
import MagicString from 'magic-string'
import { readFileSync } from 'node:fs'

const PERM_RE = /\bPermissions\.([A-Za-z_$][\w$]*)/g
const CALL_RE = /\bApiClient\.([A-Za-z_$][\w$]*)\.([A-Za-z_$][\w$]*)\s*\(/g
// A bare `ApiClient.<NS>` namespace reference (NOT `ApiClient.NS.method` — the
// negative lookahead excludes a following `.word`) — e.g. passing a whole
// namespace as an adapter. Inlined to an object literal of its methods.
const NS_RE = /\bApiClient\.([A-Za-z_$][\w$]*)(?!\s*\.\s*[A-Za-z_$])/g
const IMPORT_RE = /import\s+(type\s+)?\{([^}]*)\}\s+from\s+(['"])([^'"]*)\3\s*;?/g

/** Build an inline object literal for a whole `ApiClient.<ns>` namespace:
 *  `{ method: (...a) => callAsync("METHOD /url", ...a), ... }`. */
function namespaceLiteral(ns, endpoints) {
  const methods = []
  for (const [key, url] of endpoints) {
    const dot = key.indexOf('.')
    if (key.slice(0, dot) !== ns) continue
    const m = key.slice(dot + 1)
    methods.push(`${JSON.stringify(m)}: (...a) => callAsync(${JSON.stringify(url)}, ...a)`)
  }
  return methods.length ? `{ ${methods.join(', ')} }` : null
}

/** Parse `export enum Permissions { Member = "perm::string", ... }`. */
function loadPermissionMap(permsPath) {
  const src = readFileSync(permsPath, 'utf8')
  const map = new Map()
  const re = /([A-Za-z_$][\w$]*)\s*=\s*['"]([^'"]+)['"]/g
  let m
  while ((m = re.exec(src))) map.set(m[1], m[2])
  return map
}

/** Parse `export const ApiEndpoints = { 'NS.method': 'METHOD /url', ... }`. */
function loadEndpointMap(endpointsPath) {
  let src
  try {
    src = readFileSync(endpointsPath, 'utf8')
  } catch {
    return null // apiEndpoints.ts not split out yet → ApiClient inlining off
  }
  const map = new Map()
  const re = /'([A-Za-z0-9_]+\.[A-Za-z0-9_]+)':\s*'([A-Z]+ [^']+)'/g
  let m
  while ((m = re.exec(src))) map.set(m[1], m[2])
  return map.size ? map : null
}

/** Remove a now-unused named import specifier `sym` from the module `spec`,
 *  splitting it off its import list. Edits `s` (a MagicString) using ranges in
 *  the original `code`. */
function stripImport(s, code, sym, spec) {
  IMPORT_RE.lastIndex = 0
  let im
  while ((im = IMPORT_RE.exec(code))) {
    if (im[4] !== spec) continue
    const names = im[2].split(',').map(x => x.trim()).filter(Boolean)
    if (!names.includes(sym)) continue
    const kept = names.filter(n => n !== sym)
    const repl = kept.length
      ? `import ${im[1] || ''}{ ${kept.join(', ')} } from ${im[3]}${im[4]}${im[3]};`
      : ''
    s.overwrite(im.index, im.index + im[0].length, repl)
  }
}

export function inlineApiPlugin({ permissionsPath, endpointsPath }) {
  let perms = null
  let endpoints = null
  const ensure = () => {
    if (!perms) perms = loadPermissionMap(permissionsPath)
    if (endpoints === null) endpoints = loadEndpointMap(endpointsPath) ?? false
  }
  return {
    name: 'inline-generated-maps',
    enforce: 'pre',
    buildStart() {
      perms = loadPermissionMap(permissionsPath)
      endpoints = loadEndpointMap(endpointsPath) ?? false
    },
    transform(code, id) {
      const clean = id.split('?')[0]
      if (!/\.(ts|tsx|mts|js|jsx)$/.test(clean)) return null
      if (clean.includes('/node_modules/')) return null
      if (/\.(test|spec)\.[tj]sx?$/.test(clean)) return null

      // The app's api-client barrel: once every ApiClient.NS.method() call is
      // inlined, `export const ApiClient = createApiClient(ApiEndpoints)` is the
      // only thing keeping the ApiEndpoints map alive (its export getter is
      // retained even when unused). tsc/vitest still need the export in SOURCE,
      // so strip its CONSTRUCTION only from the emitted build → the map
      // tree-shakes. Only when ApiClient inlining is active (endpoints present).
      if (endpoints && /\/api-client\/index\.ts$/.test(clean)) {
        const s = new MagicString(code)
        const re =
          /export\s+const\s+ApiClient\s*=\s*(?:\/\*#__PURE__\*\/\s*)?createApiClient\s*<[^>]*>\s*\(\s*ApiEndpoints\s*\)\s*;?/
        const m = re.exec(code)
        if (!m) return null
        s.overwrite(m.index, m.index + m[0].length, '')
        return { code: s.toString(), map: s.generateMap({ hires: true }) }
      }
      if (/\/api-client\/(types|permissions|apiEndpoints)\.ts$/.test(clean)) return null
      const hasPerm = code.includes('Permissions.')
      const hasApi = endpoints && code.includes('ApiClient.')
      if (!hasPerm && !hasApi) return null
      ensure()

      const s = new MagicString(code)
      let didPerm = false
      let didApi = false

      if (hasPerm) {
        PERM_RE.lastIndex = 0
        let m
        while ((m = PERM_RE.exec(code))) {
          const lit = perms.get(m[1])
          if (lit == null) continue
          s.overwrite(m.index, m.index + m[0].length, JSON.stringify(lit))
          didPerm = true
        }
      }
      if (hasApi) {
        CALL_RE.lastIndex = 0
        let m
        while ((m = CALL_RE.exec(code))) {
          const url = endpoints.get(`${m[1]}.${m[2]}`)
          if (!url) continue
          s.overwrite(m.index, m.index + m[0].length, `callAsync(${JSON.stringify(url)}, `)
          didApi = true
        }
        // Bare `ApiClient.<NS>` (namespace passed as a value) → inline object literal.
        NS_RE.lastIndex = 0
        while ((m = NS_RE.exec(code))) {
          const lit = namespaceLiteral(m[1], endpoints)
          if (!lit) continue
          s.overwrite(m.index, m.index + m[0].length, lit)
          didApi = true
        }
      }
      if (!didPerm && !didApi) return null

      const after = s.toString()
      // Remove import iff the symbol no longer appears OUTSIDE import statements
      // (keeps `Permissions` where the lazy picker does `Object.entries(Permissions)`).
      const nonImport = after.replace(IMPORT_RE, '')
      if (didPerm && !/\bPermissions\b/.test(nonImport)) {
        stripImport(s, code, 'Permissions', '@/api-client/permissions')
      }
      if (didApi && !/\bApiClient\b/.test(nonImport)) {
        stripImport(s, code, 'ApiClient', '@/api-client')
      }
      if (didApi && !/import\s*\{[^}]*\bcallAsync\b[^}]*\}\s*from/.test(code)) {
        s.prepend(`import { callAsync } from '@ziee/framework/api-client';\n`)
      }
      return { code: s.toString(), map: s.generateMap({ hires: true }) }
    },
  }
}
