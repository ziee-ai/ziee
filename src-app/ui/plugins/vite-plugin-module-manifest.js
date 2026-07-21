/**
 * vite-plugin-module-manifest — the build half of "smart module loading".
 *
 * For every `module.tsx` under `src/modules/**`, it statically extracts the
 * CHEAP decision layer — `{ name, shouldLoad, routePaths, dependencies }` — from
 * the `export default createModule({ ... })` call WITHOUT evaluating the module's
 * imports, and emits a virtual module `virtual:ziee-module-manifest` exporting
 * an array of `{ name, shouldLoad?, routePaths, dependencies, load }` where
 * `load()` is a dynamic import of the heavy body.
 *
 * The decision layer is tiny, so it rides the entry chunk; each module body is a
 * separate chunk pulled only when the loader selects it. `shouldLoad` is lifted
 * verbatim, so it may reference ONLY its `ctx` param and the whitelisted
 * `Permissions` enum — any other free identifier is a hard BUILD ERROR (that's
 * what makes lifting the predicate into the entry safe).
 */

import fs from 'node:fs'
import path from 'node:path'
import fg from 'fast-glob'
import { parse } from '@babel/parser'

const VIRTUAL_ID = 'virtual:ziee-module-manifest'
const RESOLVED_ID = '\0' + VIRTUAL_ID

// JS globals a predicate may legitimately reference besides `ctx`/`Permissions`.
const ALLOWED_GLOBALS = new Set([
  'Boolean', 'Array', 'Object', 'String', 'Number', 'Math', 'JSON',
  'undefined', 'NaN', 'Infinity',
])

/** Collect the names bound by a param pattern (handles destructuring). */
function collectParamNames(node, out) {
  if (!node) return
  switch (node.type) {
    case 'Identifier': out.add(node.name); return
    case 'ObjectPattern': node.properties.forEach(p => collectParamNames(p.value ?? p.argument, out)); return
    case 'ArrayPattern': node.elements.forEach(e => collectParamNames(e, out)); return
    case 'AssignmentPattern': collectParamNames(node.left, out); return
    case 'RestElement': collectParamNames(node.argument, out); return
  }
}

/** Collect free (unbound) reference-position identifier ROOTS within a node. */
function collectFreeRefs(node, bound, out) {
  if (!node || typeof node !== 'object') return
  switch (node.type) {
    case 'Identifier':
      if (!bound.has(node.name)) out.add(node.name)
      return
    case 'MemberExpression':
      collectFreeRefs(node.object, bound, out)
      if (node.computed) collectFreeRefs(node.property, bound, out) // ctx[x] — property IS a ref
      return // non-computed `.prop` is not a free ref
    case 'ArrowFunctionExpression':
    case 'FunctionExpression': {
      const inner = new Set(bound)
      node.params.forEach(p => collectParamNames(p, inner))
      collectFreeRefs(node.body, inner, out)
      return
    }
    case 'ObjectProperty':
    case 'Property':
      if (node.computed) collectFreeRefs(node.key, bound, out)
      collectFreeRefs(node.value, bound, out)
      return
    default:
      for (const k in node) {
        if (k === 'type' || k === 'start' || k === 'end' || k === 'loc' || k === 'range' || k === 'leadingComments' || k === 'trailingComments') continue
        const v = node[k]
        if (Array.isArray(v)) v.forEach(c => collectFreeRefs(c, bound, out))
        else if (v && typeof v === 'object' && v.type) collectFreeRefs(v, bound, out)
      }
  }
}

/**
 * Validate a lifted `shouldLoad` arrow fn is pure over `ctx` + `Permissions`.
 * Returns `{ usesPermissions }` or throws with a precise, actionable message.
 */
function checkPurity(fnNode, file) {
  const bound = new Set()
  fnNode.params.forEach(p => collectParamNames(p, bound))
  const free = new Set()
  collectFreeRefs(fnNode.body, bound, free)
  let usesPermissions = false
  const illegal = []
  for (const name of free) {
    if (name === 'Permissions') { usesPermissions = true; continue }
    if (ALLOWED_GLOBALS.has(name)) continue
    illegal.push(name)
  }
  if (illegal.length) {
    throw new Error(
      `[module-manifest] shouldLoad in ${path.relative(process.cwd(), file)} references ${illegal
        .map(n => `\`${n}\``)
        .join(', ')} — a shouldLoad predicate may only use its \`ctx\` param and the \`Permissions\` enum (gate permissions via \`ctx.can(Permissions.X)\`). Move any other logic out of shouldLoad.`,
    )
  }
  return { usesPermissions }
}

/** Extract the manifest fields from one module.tsx source. */
function extractModule(file, src) {
  const ast = parse(src, { sourceType: 'module', plugins: ['typescript', 'jsx'] })
  let obj = null
  for (const node of ast.program.body) {
    if (node.type === 'ExportDefaultDeclaration') {
      const d = node.declaration
      if (
        d.type === 'CallExpression' &&
        d.callee.name === 'createModule' &&
        d.arguments[0]?.type === 'ObjectExpression'
      ) {
        obj = d.arguments[0]
      }
    }
  }
  if (!obj) return null // not a createModule default export — skip
  const prop = k => obj.properties.find(p => p.key && (p.key.name === k || p.key.value === k))

  // metadata.name
  let name = null
  const meta = prop('metadata')
  if (meta?.value?.type === 'ObjectExpression') {
    const n = meta.value.properties.find(p => p.key?.name === 'name')
    if (n?.value?.type === 'StringLiteral') name = n.value.value
  }
  if (!name) throw new Error(`[module-manifest] ${path.relative(process.cwd(), file)}: createModule has no static metadata.name`)

  // shouldLoad — lift source + purity check
  const sl = prop('shouldLoad')
  let shouldLoadSrc = null
  let usesPermissions = false
  if (sl) {
    if (sl.value.type !== 'ArrowFunctionExpression' && sl.value.type !== 'FunctionExpression') {
      throw new Error(`[module-manifest] ${name}: shouldLoad must be an (arrow) function literal so it can be lifted into the manifest`)
    }
    shouldLoadSrc = src.slice(sl.value.start, sl.value.end)
    usesPermissions = checkPurity(sl.value, file).usesPermissions
  }

  // routes[].path string literals
  const routes = prop('routes')
  const routePaths = []
  if (routes?.value?.type === 'ArrayExpression') {
    for (const el of routes.value.elements) {
      if (el?.type === 'ObjectExpression') {
        const p = el.properties.find(pr => pr.key?.name === 'path')
        if (p?.value?.type === 'StringLiteral') routePaths.push(p.value.value)
      }
    }
  }

  // dependencies
  const deps = prop('dependencies')
  const dependencies =
    deps?.value?.type === 'ArrayExpression'
      ? deps.value.elements.filter(e => e?.type === 'StringLiteral').map(e => e.value)
      : []

  return { name, shouldLoadSrc, usesPermissions, routePaths, dependencies }
}

/**
 * @param {{ srcDir: string }} opts — absolute path to `src` (for the `@/`
 *   import-path rewrite the generated `load()` uses).
 */
export function moduleManifestPlugin(opts) {
  const srcDir = opts.srcDir
  const modulesGlob = path.join(srcDir, 'modules/**/module.tsx').replace(/\\/g, '/')

  function buildManifestSource() {
    const files = fg.sync(modulesGlob, { absolute: true })
    const entries = []
    let anyPermissions = false
    for (const file of files) {
      const src = fs.readFileSync(file, 'utf8')
      const ex = extractModule(file, src)
      if (!ex) continue
      if (ex.usesPermissions) anyPermissions = true
      const importPath = '@/' + path.relative(srcDir, file).replace(/\\/g, '/')
      entries.push({ ...ex, importPath })
    }
    // deterministic order (by name) — the loader re-sorts by dependencies anyway
    entries.sort((a, b) => a.name.localeCompare(b.name))

    const lines = entries.map(e => {
      const parts = [`name: ${JSON.stringify(e.name)}`]
      if (e.shouldLoadSrc) parts.push(`shouldLoad: ${e.shouldLoadSrc}`)
      parts.push(`routePaths: ${JSON.stringify(e.routePaths)}`)
      parts.push(`dependencies: ${JSON.stringify(e.dependencies)}`)
      parts.push(`load: () => import(${JSON.stringify(e.importPath)})`)
      return `  { ${parts.join(', ')} },`
    })

    return (
      (anyPermissions ? `import { Permissions } from '@/api-client/permissions'\n\n` : '') +
      `// AUTO-GENERATED by vite-plugin-module-manifest. Do not edit.\n` +
      `export const manifest = [\n${lines.join('\n')}\n]\n`
    )
  }

  return {
    name: 'ziee-module-manifest',
    enforce: 'pre',
    resolveId(id) {
      if (id === VIRTUAL_ID) return RESOLVED_ID
    },
    load(id) {
      if (id === RESOLVED_ID) return buildManifestSource()
    },
    // HMR: a module.tsx add/remove/metadata change must rebuild the manifest.
    handleHotUpdate(ctx) {
      if (/[\\/]modules[\\/].*[\\/]module\.tsx$/.test(ctx.file)) {
        const mod = ctx.server.moduleGraph.getModuleById(RESOLVED_ID)
        if (mod) ctx.server.moduleGraph.invalidateModule(mod)
      }
    },
  }
}
