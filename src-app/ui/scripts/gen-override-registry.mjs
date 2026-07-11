/**
 * Generate OVERRIDE_MANIFEST.md — the living index of every desktop UI override
 * seam, and drift-guard the override surface.
 *
 * Scans both trees for:
 *   - DECLARED seams   — `interface UIOverrides { 'key': … }` augmentations + the
 *                        `<Seam id="key">` / `useOverride('key', …)` usages in core (ui/src).
 *   - REGISTERED overrides — `registerOverride('key', …)` calls in desktop (desktop/ui/src).
 *   - `.desktop.tsx|ts` co-located whole-file overrides in the CORE tree.
 *
 * --check FAILS on:
 *   (a) a `registerOverride('key')` whose key is NOT a declared seam  (dead override), and
 *   (b) an orphaned `*.desktop.{tsx,ts}` with no core sibling of the same base name.
 * A declared-but-unregistered seam is legitimate (web-only so far) → reported, not failed.
 *
 * Run: node scripts/gen-override-registry.mjs        (write manifest)
 *      node scripts/gen-override-registry.mjs --check (drift + dead-override guard)
 */
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const UI_SRC = path.resolve(HERE, '../src')
const DESKTOP_SRC = path.resolve(HERE, '../../desktop/ui/src')
const OUT = path.join(UI_SRC, 'core/overrides/OVERRIDE_MANIFEST.md')

const DESKTOP_INFIX = /\.desktop\.(tsx|ts)$/

function walk(dir, acc = []) {
  if (!fs.existsSync(dir)) return acc
  for (const e of fs.readdirSync(dir)) {
    const full = path.join(dir, e)
    if (fs.statSync(full).isDirectory()) {
      if (!['node_modules', 'dist', 'build', '.git'].includes(e)) walk(full, acc)
    } else if (/\.(tsx|ts)$/.test(e) && !/\.test\.(tsx|ts)$/.test(e)) {
      acc.push(full)
    }
  }
  return acc
}

const rel = (f) => path.relative(path.resolve(HERE, '../../..'), f)

// Strip block + line comments so JSDoc EXAMPLES (which legitimately show
// `<Seam id="…">` / `interface UIOverrides { … }`) don't pollute the scan. Naive
// but safe here: over-stripping a `//`-in-a-string can't invent a seam key.
function stripComments(src) {
  return src
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:])\/\/[^\n]*/g, '$1')
}

// --- collect declared seams (keys inside `interface UIOverrides { … }`) -------
const IFACE = /interface\s+UIOverrides\s*\{([\s\S]*?)\n\s*\}/g
const KEY_IN_FACE = /['"]([^'"]+)['"]\s*:/g
const SEAM_USE = /<Seam\s+[^>]*\bid=['"]([^'"]+)['"]/g
const USE_OVERRIDE = /useOverride\(\s*['"]([^'"]+)['"]/g
const REGISTER = /registerOverride\(\s*['"]([^'"]+)['"]/g

const declared = new Map() // key -> file
const used = new Map() // key -> file
const registered = new Map() // key -> file
const desktopFiles = [] // { file, base, hasSibling }

function scan(root, isCore) {
  for (const f of walk(root)) {
    const src = stripComments(fs.readFileSync(f, 'utf-8'))
    if (DESKTOP_INFIX.test(f)) {
      const base = f.replace(DESKTOP_INFIX, '') // strip `.desktop.tsx`
      const hasSibling =
        fs.existsSync(base + '.tsx') || fs.existsSync(base + '.ts')
      desktopFiles.push({ file: f, hasSibling })
    }
    let m
    IFACE.lastIndex = 0
    while ((m = IFACE.exec(src)) !== null) {
      const body = m[1]
      let k
      KEY_IN_FACE.lastIndex = 0
      while ((k = KEY_IN_FACE.exec(body)) !== null) {
        if (!declared.has(k[1])) declared.set(k[1], rel(f))
      }
    }
    for (const [re, map] of [
      [SEAM_USE, used],
      [USE_OVERRIDE, used],
      [REGISTER, registered],
    ]) {
      re.lastIndex = 0
      while ((m = re.exec(src)) !== null) {
        if (!map.has(m[1])) map.set(m[1], rel(f))
      }
    }
  }
}

/**
 * Pure drift analysis (exported for TEST-7). `declared`/`registered` are
 * key→file Maps; `desktopFiles` is `{ file, hasSibling }[]`.
 *   - deadOverrides: a registerOverride key with no declared seam.
 *   - orphanDesktopFiles: a `*.desktop.*` with no core sibling.
 *   - unregisteredSeams: a declared seam with no desktop override (legitimate).
 */
export function computeDrift(declared, registered, desktopFiles) {
  return {
    deadOverrides: [...registered.keys()].filter((k) => !declared.has(k)),
    orphanDesktopFiles: desktopFiles.filter((d) => !d.hasSibling),
    unregisteredSeams: [...declared.keys()].filter((k) => !registered.has(k)),
  }
}

const isMain = import.meta.url === `file://${process.argv[1]}`
if (isMain) {
scan(UI_SRC, true)
scan(DESKTOP_SRC, false)

// --- drift checks -------------------------------------------------------------
const { deadOverrides, orphanDesktopFiles, unregisteredSeams } = computeDrift(
  declared,
  registered,
  desktopFiles,
)

// --- manifest -----------------------------------------------------------------
const seamKeys = [...declared.keys()].sort()
const rows = seamKeys
  .map((k) => {
    const reg = registered.get(k)
    return `| \`${k}\` | ${declared.get(k)} | ${used.get(k) ?? '—'} | ${reg ? `✓ ${reg}` : '— (web-only)'} |`
  })
  .join('\n')

const relocRows = desktopFiles
  .sort((a, b) => a.file.localeCompare(b.file))
  .map((d) => `| \`${rel(d.file)}\` | ${d.hasSibling ? '✓' : '❌ ORPHAN'} |`)
  .join('\n')

const body = `<!-- AUTO-GENERATED by scripts/gen-override-registry.mjs — DO NOT EDIT. -->
# Desktop UI Override Manifest

The living index of every desktop UI override. Regenerate with
\`node scripts/gen-override-registry.mjs\`; \`--check\` runs in \`npm run check\`.

## Element-level seams (\`<Seam>\` / \`useOverride\`)

${seamKeys.length} declared seam${seamKeys.length === 1 ? '' : 's'}.

| Seam key | Declared in | Used in | Desktop override |
|---|---|---|---|
${rows || '| _(none)_ | | | |'}

## Whole-file overrides (\`.desktop.{tsx,ts}\` co-located in the core tree)

${desktopFiles.length} co-located override file${desktopFiles.length === 1 ? '' : 's'}.

| File | Core sibling |
|---|---|
${relocRows || '| _(none)_ | |'}
`

const check = process.argv.includes('--check')
let failed = false

if (deadOverrides.length) {
  failed = true
  console.error(
    `override drift: registerOverride() for undeclared seam(s): ${deadOverrides
      .map((k) => `'${k}' (${registered.get(k)})`)
      .join(', ')} — declare the seam in the core component or remove the registration.`,
  )
}
if (orphanDesktopFiles.length) {
  failed = true
  console.error(
    `override drift: orphaned .desktop file(s) with no core sibling: ${orphanDesktopFiles
      .map((d) => rel(d.file))
      .join(', ')}`,
  )
}

if (check) {
  const cur = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf-8') : ''
  if (cur.trim() !== body.trim()) {
    failed = true
    console.error(
      'OVERRIDE_MANIFEST.md is stale — run `node scripts/gen-override-registry.mjs` and commit.',
    )
  }
  if (failed) process.exit(1)
  console.log(
    `override registry OK — ${seamKeys.length} seam(s), ${desktopFiles.length} .desktop file(s), ${unregisteredSeams.length} web-only.`,
  )
} else {
  fs.writeFileSync(OUT, body)
  console.log(
    `Wrote ${OUT} — ${seamKeys.length} seam(s), ${desktopFiles.length} .desktop file(s).`,
  )
  if (failed) process.exit(1)
}
}
