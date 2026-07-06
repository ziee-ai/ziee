/**
 * Advisory lint (Layer 3, taxonomy J8): raw native scroll where the shared kit
 * scroll wrapper belongs. The app ships `src/components/common/DivScrollY.tsx`
 * (OverlayScrollbars auto-hide, consistent cross-platform look). A raw JSX host
 * element (div/section/main/…) with `overflow-y-auto` / `overflow-y-scroll` /
 * `overflow-auto` / `overflow-scroll` in its className bypasses it — the message
 * list on desktop is exactly this (user miss #17: native scrollbar instead of
 * DivScrollY).
 *
 * This is ADVISORY (reports, exit 0) not gating: the codebase has a backlog of
 * pre-existing raw-scroll sites; wire `--gate` once they're burned down or
 * grandfathered in native-scroll-allowlist.json. Genuine exceptions (textarea,
 * <pre>/<code>, a horizontal-only strip) opt out with `data-allow-scroll`.
 *
 *   node scripts/lint-native-scroll.mjs [--gate]
 */
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const HERE = path.dirname(fileURLToPath(import.meta.url))
const ROOTS = [path.resolve(HERE, '../src'), path.resolve(HERE, '../../desktop/ui/src')]
const ALLOWLIST = path.resolve(HERE, '../src/dev/gallery/native-scroll-allowlist.json')
const GATE = process.argv.includes('--gate')
const OPT_OUT = 'data-allow-scroll'

const SCROLL = /\boverflow-y-auto\b|\boverflow-y-scroll\b|\boverflow-auto\b|\boverflow-scroll\b/
// Host tags where a native scrollbar means "should have used DivScrollY". <pre>,
// <code>, <textarea> legitimately scroll their own content.
const EXEMPT_TAGS = new Set(['pre', 'code', 'textarea'])

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
    } else if (/\.tsx$/.test(e) && !e.endsWith('.generated.ts') && !/DivScroll/.test(e)) acc.push(full)
  }
  return acc
}

function classNameOf(node) {
  const el = ts.isJsxElement(node) ? node.openingElement : node
  for (const a of el.attributes?.properties ?? []) {
    if (!ts.isJsxAttribute(a) || a.name?.getText() !== 'className') continue
    let out = ''
    const collect = n => {
      if (ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n)) out += ' ' + n.text
      ts.forEachChild(n, collect)
    }
    if (a.initializer) collect(a.initializer)
    return out
  }
  return ''
}
const hasAttr = (node, name) => {
  const el = ts.isJsxElement(node) ? node.openingElement : node
  return (el.attributes?.properties ?? []).some(a => ts.isJsxAttribute(a) && a.name?.getText() === name)
}
const tagName = node => {
  const el = ts.isJsxElement(node) ? node.openingElement : ts.isJsxSelfClosingElement(node) ? node : null
  if (!el) return null
  const n = el.tagName
  return ts.isIdentifier(n) ? n.text : n.getText()
}

const findings = []
for (const root of ROOTS) {
  for (const file of findFiles(root)) {
    const sf = ts.createSourceFile(file, fs.readFileSync(file, 'utf-8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX)
    const visit = node => {
      if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) {
        const tag = tagName(node)
        // raw host element only (lowercase tag name); components manage their own.
        if (tag && /^[a-z]/.test(tag) && !EXEMPT_TAGS.has(tag) && !hasAttr(node, OPT_OUT)) {
          if (SCROLL.test(classNameOf(node))) {
            const allowed = allowEntries.some(e => e.file && sf.fileName.includes(e.file) && (e.line == null))
            if (!allowed) {
              const { line } = sf.getLineAndCharacterOfPosition((ts.isJsxElement(node) ? node.openingElement : node).getStart(sf))
              findings.push({ file: sf.fileName, line: line + 1, tag })
            }
          }
        }
      }
      ts.forEachChild(node, visit)
    }
    visit(sf)
  }
}

if (findings.length) {
  console.log(`[native-scroll] ${findings.length} raw native-scroll site(s) that should use <DivScrollY> (advisory):\n`)
  for (const f of findings.slice(0, 60))
    console.log(`  ${path.relative(process.cwd(), f.file)}:${f.line}  <${f.tag}> with native overflow scroll`)
  if (findings.length > 60) console.log(`  … +${findings.length - 60} more`)
  console.log(`\nUse <DivScrollY> (src/components/common) for a consistent auto-hide scrollbar, or add \`${OPT_OUT}\` for a genuine exception.`)
  if (GATE) process.exit(1)
} else console.log('[native-scroll] ✓ no raw native-scroll sites.')
