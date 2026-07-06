/**
 * Guardrail (Layer 3): action-name ↔ expected-icon mapping (taxonomy C11).
 *
 * An icon-only control communicates its action through ONE thing: its glyph. If a
 * "download" button renders a Copy icon, or "open in new tab" renders a bare arrow,
 * the affordance lies. This lint reads each icon-bearing control's accessible name
 * (aria-label / tooltip / label / visible text), and if that name maps to a known
 * action, asserts the lucide icon it renders matches the expected glyph.
 *
 * Mapping is deliberately small + high-confidence; a justified deviation opts out
 * with `data-allow-icon` on the element (or an entry in icon-action-allowlist.json).
 * TS-compiler-API AST. kit/ + shadcn/ excluded.
 *
 *   node scripts/lint-icon-action.mjs
 */
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const HERE = path.dirname(fileURLToPath(import.meta.url))
const ROOTS = [path.resolve(HERE, '../src'), path.resolve(HERE, '../../desktop/ui/src')]
const ALLOWLIST = path.resolve(HERE, '../src/dev/gallery/icon-action-allowlist.json')
const OPT_OUT = 'data-allow-icon'

// action keyword (matched in the accessible name, word-ish) → acceptable lucide icons
const MAP = {
  // cancel/dismiss FIRST so "cancel edit" resolves to cancel (X), not edit.
  cancel: ['X', 'XCircle'],
  dismiss: ['X', 'XCircle'],
  'open in new tab': ['ExternalLink', 'SquareArrowOutUpRight'],
  'new tab': ['ExternalLink', 'SquareArrowOutUpRight'],
  download: ['Download', 'DownloadCloud'],
  delete: ['Trash2', 'Trash'],
  remove: ['Trash2', 'Trash', 'X', 'Minus', 'CircleMinus', 'MinusCircle'],
  copy: ['Copy', 'ClipboardCopy'],
  edit: ['Pencil', 'PencilLine', 'Edit', 'Edit2', 'Edit3'],
  close: ['X', 'XCircle'],
  refresh: ['RotateCw', 'RefreshCw', 'RotateCcw'],
  settings: ['Settings', 'Settings2', 'Sliders'],
}
const KEYS = Object.keys(MAP)

const allow = fs.existsSync(ALLOWLIST) ? JSON.parse(fs.readFileSync(ALLOWLIST, 'utf8')) : { entries: [] }
const allowEntries = Array.isArray(allow) ? allow : allow.entries || []

function findFiles(dir, acc = []) {
  if (!fs.existsSync(dir)) return acc
  for (const e of fs.readdirSync(dir)) {
    const full = path.join(dir, e)
    const st = fs.statSync(full)
    if (st.isDirectory()) {
      if (!['node_modules', 'dist', 'build', '.git', 'tests'].includes(e) &&
        !full.endsWith(path.join('components', 'ui', 'kit')) &&
        !full.endsWith(path.join('components', 'ui', 'shadcn')))
        findFiles(full, acc)
    } else if (/\.tsx$/.test(e) && !e.endsWith('.generated.ts')) acc.push(full)
  }
  return acc
}

const tagName = node => {
  const el = ts.isJsxElement(node) ? node.openingElement : ts.isJsxSelfClosingElement(node) ? node : null
  if (!el) return null
  const n = el.tagName
  return ts.isIdentifier(n) ? n.text : n.getText()
}
// string value of a named string/tooltip attribute
function strAttr(node, name) {
  const el = ts.isJsxElement(node) ? node.openingElement : node
  for (const a of el.attributes?.properties ?? []) {
    if (!ts.isJsxAttribute(a) || a.name?.getText() !== name) continue
    const init = a.initializer
    if (!init) return ''
    if (ts.isStringLiteral(init)) return init.text
    if (ts.isJsxExpression(init) && init.expression && ts.isStringLiteral(init.expression)) return init.expression.text
  }
  return ''
}
function hasAttr(node, name) {
  const el = ts.isJsxElement(node) ? node.openingElement : node
  return (el.attributes?.properties ?? []).some(a => ts.isJsxAttribute(a) && a.name?.getText() === name)
}
// lucide icon identifiers referenced anywhere inside the element (icon= prop or children)
function iconsIn(node) {
  const names = new Set()
  const collect = n => {
    if (ts.isJsxSelfClosingElement(n) || ts.isJsxElement(n)) {
      const t = tagName(n)
      if (t && /^[A-Z]/.test(t)) names.add(t)
    }
    ts.forEachChild(n, collect)
  }
  const el = ts.isJsxElement(node) ? node.openingElement : node
  for (const a of el.attributes?.properties ?? []) if (ts.isJsxAttribute(a) && a.name?.getText() === 'icon') collect(a)
  if (ts.isJsxElement(node)) for (const c of node.children) collect(c)
  return names
}

const violations = []
const fileViewerIcons = []

for (const root of ROOTS) {
  for (const file of findFiles(root)) {
    const src = fs.readFileSync(file, 'utf-8')
    const sf = ts.createSourceFile(file, src, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX)
    // lucide imports in this file (so we only compare against real lucide icons)
    const lucide = new Set()
    const importVisit = n => {
      if (ts.isImportDeclaration(n) && /lucide-react/.test(n.moduleSpecifier.getText()) && n.importClause?.namedBindings && ts.isNamedImports(n.importClause.namedBindings))
        for (const s of n.importClause.namedBindings.elements) lucide.add(s.name.text)
      ts.forEachChild(n, importVisit)
    }
    importVisit(sf)

    const visit = node => {
      const tag = tagName(node)
      if (tag && /Button$/.test(tag)) {
        const name = (strAttr(node, 'aria-label') || strAttr(node, 'tooltip') || strAttr(node, 'label') || '').toLowerCase()
        const tid = strAttr(node, 'data-testid')
        const icons = [...iconsIn(node)].filter(i => lucide.has(i))
        // report file-viewer open-in-new-tab icon specifically (acceptance #10b)
        if (/open.*new.*tab|new-tab/.test(tid + ' ' + name) && icons.length)
          fileViewerIcons.push({ file: sf.fileName, tid, icons })
        if (name && !hasAttr(node, OPT_OUT)) {
          const key = KEYS.find(k => name.includes(k))
          if (key && icons.length) {
            const ok = icons.some(i => MAP[key].includes(i))
            const allowed = allowEntries.some(e => (e.testid && tid.includes(e.testid)) || (e.file && sf.fileName.includes(e.file) && (!e.action || e.action === key)))
            if (!ok && !allowed) {
              const { line } = sf.getLineAndCharacterOfPosition((ts.isJsxElement(node) ? node.openingElement : node).getStart(sf))
              violations.push({ file: sf.fileName, line: line + 1, name, key, icons, expected: MAP[key] })
            }
          }
        }
      }
      ts.forEachChild(node, visit)
    }
    visit(sf)
  }
}

if (fileViewerIcons.length) {
  console.log('[icon-action] file-viewer "open in new tab" icon(s) in use:')
  for (const f of fileViewerIcons) console.log(`  ${path.relative(process.cwd(), f.file)}  testid=${f.tid}  icon=${f.icons.join(',')}`)
}
if (violations.length) {
  console.error(`\n[icon-action] ✗ ${violations.length} control(s) whose icon doesn't match its action:\n`)
  for (const v of violations)
    console.error(`  ${path.relative(process.cwd(), v.file)}:${v.line}  "${v.name}" (${v.key}) renders {${v.icons.join(',')}}, expected one of {${v.expected.join(',')}}`)
  console.error(`\nUse the conventional icon, or add \`${OPT_OUT}\` / an icon-action-allowlist.json entry with a reason.\n`)
  process.exit(1)
}
console.log('[icon-action] ✓ icon-bearing controls use their conventional action glyph.')
