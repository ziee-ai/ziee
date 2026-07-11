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
const REPO = path.resolve(HERE, '../../..')

// ── Raw-shadow gate ──────────────────────────────────────────────────────────
// A "raw whole-file shadow" is a desktop-tree file `desktop/ui/src/X` whose core
// sibling `ui/src/X` exists — the coarse override we're eliminating. After
// migration a shadow must be gone (replaced by a co-located `X.desktop.tsx` or a
// `<Seam>`); the only raw shadow allowed is one recorded as an approved
// SHADOW-EXCEPTION in DECISIONS.md (main.tsx entry, glob-discovered module.tsx,
// generated types). Desktop-EXCLUSIVE files (no core sibling) are legit modules.

/** Desktop-tree source files (excluding tests/dev/gallery/generated). */
function walkDesktopSrc(dir, acc = []) {
  if (!fs.existsSync(dir)) return acc
  for (const e of fs.readdirSync(dir)) {
    const full = path.join(dir, e)
    if (fs.statSync(full).isDirectory()) {
      if (!['node_modules', 'dist', 'build', '.git', 'dev', '__detector_fixtures__'].includes(e))
        walkDesktopSrc(full, acc)
    } else if (
      /\.(tsx|ts)$/.test(e) &&
      !/\.(test|spec)\.(tsx|ts)$/.test(e) &&
      !/\.generated\.(tsx|ts)$/.test(e)
    ) {
      acc.push(full)
    }
  }
  return acc
}

/** Every desktop-tree file, split into raw shadows (ui sibling exists) vs
 *  desktop-exclusive (no ui sibling). Paths are relative to `desktop/ui/src`. */
export function enumerateDesktopFiles(uiSrc = UI_SRC, desktopSrc = DESKTOP_SRC) {
  const shadows = []
  const exclusive = []
  for (const abs of walkDesktopSrc(desktopSrc)) {
    const relPath = path.relative(desktopSrc, abs).replace(/\\/g, '/')
    if (fs.existsSync(path.join(uiSrc, relPath))) shadows.push(relPath)
    else exclusive.push(relPath)
  }
  return { shadows: shadows.sort(), exclusive: exclusive.sort() }
}

/** Approved raw-shadow exceptions from DECISIONS.md. Format:
 *  `- SHADOW-EXCEPTION: <path> — <reason> [approved: <who/when>]` */
export function parseShadowExceptions(decisionsText) {
  const set = new Set()
  // Path is a single whitespace-delimited token (may contain `-`, `/`, `.`),
  // then a ` — `/` - ` separator, then the reason + [approved: …].
  const re = /^-\s*SHADOW-EXCEPTION:\s*(\S+)\s+[—-]\s.*\[approved:/gim
  let m
  while ((m = re.exec(decisionsText || '')) !== null) set.add(m[1].trim())
  return set
}


// Strip block + line comments so JSDoc EXAMPLES (which legitimately show
// `<Seam id="…">` / `interface UIOverrides { … }`) don't pollute the scan. Naive
// but safe here: over-stripping a `//`-in-a-string can't invent a seam key.
function stripComments(src) {
  return src
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:])\/\/[^\n]*/g, '$1')
}

// --- collect declared seams (keys inside `interface UIOverrides { … }`) -------
// Brace-BALANCED extraction: a seam value may be a multi-line object type
// (`'x.y': { a: number\n  b: string }`), so a non-greedy `}`-terminated regex
// would truncate the body and also capture NESTED quoted keys. Scan the true
// interface body by depth, and take only DEPTH-0 keys.
export function topLevelSeamKeys(src) {
  const keys = []
  const open = /interface\s+UIOverrides\s*\{/g
  let m
  while ((m = open.exec(src)) !== null) {
    let i = m.index + m[0].length // just past the opening `{`
    const start = i
    let depth = 1
    while (i < src.length && depth > 0) {
      const c = src[i++]
      if (c === '{') depth++
      else if (c === '}') depth--
    }
    const body = src.slice(start, i - 1)
    let d = 0
    const tok = /['"]([^'"]+)['"]\s*:|[{}]/g
    let t
    while ((t = tok.exec(body)) !== null) {
      if (t[0] === '{') d++
      else if (t[0] === '}') d--
      else if (d === 0 && t[1]) keys.push(t[1])
    }
  }
  return keys
}
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
    for (const key of topLevelSeamKeys(src)) {
      if (!declared.has(key)) declared.set(key, rel(f))
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

// --- raw-shadow gate ----------------------------------------------------------
const { shadows: desktopShadows, exclusive: desktopExclusive } =
  enumerateDesktopFiles()
const decisionsText = (() => {
  for (const p of [
    path.join(REPO, '.lifecycle/desktop-ui-override/DECISIONS.md'),
  ]) {
    if (fs.existsSync(p)) return fs.readFileSync(p, 'utf-8')
  }
  return ''
})()
const shadowExceptions = parseShadowExceptions(decisionsText)
const unaccountedShadows = desktopShadows.filter((s) => !shadowExceptions.has(s))
// Exclusive MODULE roots (for the manifest listing).
const exclusiveModules = desktopExclusive
  .filter((f) => /(^|\/)module\.tsx$/.test(f))
  .map((f) => f.replace(/\/module\.tsx$/, '').replace(/^module\.tsx$/, '.'))

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

## Approved raw whole-file shadows (structural exceptions)

Desktop-tree files that shadow a \`src-app/ui\` path AND cannot use a finer
mechanism — each requires an approved \`SHADOW-EXCEPTION\` in DECISIONS.md.

${desktopShadows.length} raw shadow${desktopShadows.length === 1 ? '' : 's'} (${unaccountedShadows.length} UNACCOUNTED).

| Shadow | Approved exception? |
|---|---|
${desktopShadows.map((s) => `| \`${s}\` | ${shadowExceptions.has(s) ? '✓' : '❌ UNACCOUNTED — migrate to a <Seam>/.desktop.tsx or record an approved SHADOW-EXCEPTION'} |`).join('\n') || '| _(none)_ | |'}

## Desktop-exclusive modules (no \`src-app/ui\` sibling — legit file modules)

${exclusiveModules.length} exclusive module${exclusiveModules.length === 1 ? '' : 's'}.

${exclusiveModules.map((m) => `- \`${m}\``).join('\n') || '- _(none)_'}
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
if (unaccountedShadows.length) {
  failed = true
  console.error(
    `override drift: ${unaccountedShadows.length} RAW whole-file shadow(s) not migrated and not approved:\n` +
      unaccountedShadows.map((s) => `    desktop/ui/src/${s}`).join('\n') +
      `\n  → convert each to a <Seam> (finest) or a co-located ui/src/${'<path>'}.desktop.tsx, ` +
      `or record an approved "- SHADOW-EXCEPTION: <path> — <structural reason> [approved: …]" in DECISIONS.md.`,
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
