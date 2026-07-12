/**
 * seam-codemod — scaffold + migrate desktop UI override seams.
 *
 *   node scripts/seam-codemod.mjs add <seam-key> <core-file>
 *       Declare a `<Seam>` in a core component + emit the desktop registration
 *       stub + refresh the manifest. `<core-file>` is repo-relative or absolute;
 *       `<seam-key>` is `<module>.<element>` kebab-case.
 *
 *   node scripts/seam-codemod.mjs migrate <desktop-shadow-file>
 *       Classify a desktop-tree shadow against its core sibling: ELEMENT-LEVEL
 *       (a localized divergence → a `<Seam>` is the right tool; scaffolds it) or
 *       STRUCTURAL (the whole component differs → keep a tier-1 shadow or use a
 *       `.desktop.tsx` file-swap; does NOT force a seam).
 *
 * Uses string/AST-light transforms to match the repo's other `.mjs` generators
 * (ts-morph is available but a full auto-rewriter is neither warranted — most
 * existing overrides are structural — nor consistent with the codebase; the
 * output is always human-reviewed regardless). Exported functions are unit-tested
 * by seam-codemod.test.mjs.
 */
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs'
import { spawnSync } from 'node:child_process'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const REPO = path.resolve(HERE, '../../..')
const UI_SRC = path.resolve(HERE, '../src')
const DESKTOP_SRC = path.resolve(HERE, '../../desktop/ui/src')
const OVERRIDES_DIR = path.join(DESKTOP_SRC, 'modules/desktop-base/overrides')

const KEY_RE = /^[a-z0-9-]+(\.[a-z0-9-]+)+$/

export const slugForKey = (key) => key.replace(/\./g, '-')

export function augmentationBlock(key) {
  return `declare module '@/core/overrides' {
  interface UIOverrides {
    // Auto-scaffolded by seam-codemod. Replace \`Record<string, never>\` with the
    // override's props type if the desktop variant needs any.
    '${key}': Record<string, never>
  }
}`
}

/**
 * Insert the seam augmentation into a core source string after the last
 * top-level import. Idempotent: returns { src, changed:false } if the key is
 * already declared.
 */
export function insertAugmentation(src, key) {
  if (new RegExp(`['"]${key.replace(/[.]/g, '\\.')}['"]\\s*:`).test(src)) {
    return { src, changed: false }
  }
  const lines = src.split('\n')
  let lastImport = -1
  for (let i = 0; i < lines.length; i++) {
    if (/^\s*import\s/.test(lines[i]) || /^\s*}\s*from\s/.test(lines[i])) {
      lastImport = i
    }
  }
  const block = augmentationBlock(key)
  const at = lastImport + 1
  lines.splice(at, 0, '', block)
  return { src: lines.join('\n'), changed: true }
}

export function registrationStub(key) {
  const slug = slugForKey(key)
  const comp = `Desktop${slug
    .split('-')
    .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
    .join('')}`
  return `/**
 * Desktop override for seam \`${key}\` (scaffolded by seam-codemod).
 * Replace the placeholder with the desktop variant, then wrap the element in a
 * <Seam id="${key}"> in the core component (leave its current markup as the
 * fallback children).
 */
import { registerOverride } from '@/core/overrides'

function ${comp}() {
  // TODO: the desktop-specific element.
  return null
}

export function register(): void {
  registerOverride('${key}' as never, ${comp})
}
`
}

/**
 * Classify a shadow vs its core sibling by how localized the divergence is.
 * Returns { classification, ratio, changed, total }. A low change ratio ⇒ a
 * localized (element-level) divergence a seam can capture; a high ratio ⇒
 * structural (file-swap territory).
 */
export function classifyDivergence(coreSrc, shadowSrc, threshold = 0.4) {
  const norm = (s) =>
    s
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith('//') && !l.startsWith('*') && l !== '/**' && l !== '*/')
  const core = norm(coreSrc)
  const shadow = norm(shadowSrc)
  const coreSet = new Set(core)
  const shadowSet = new Set(shadow)
  const added = shadow.filter((l) => !coreSet.has(l)).length
  const removed = core.filter((l) => !shadowSet.has(l)).length
  const total = Math.max(core.length, shadow.length, 1)
  const ratio = (added + removed) / total
  return {
    classification: ratio <= threshold ? 'element-level' : 'structural',
    ratio: Number(ratio.toFixed(3)),
    changed: added + removed,
    total,
  }
}

// ── CLI ──────────────────────────────────────────────────────────────────────
function resolveInput(p) {
  return path.isAbsolute(p) ? p : path.resolve(REPO, p)
}

function refreshManifest() {
  spawnSync('node', [path.join(HERE, 'gen-override-registry.mjs')], {
    stdio: 'inherit',
  })
}

function cmdAdd(key, coreFileArg) {
  if (!KEY_RE.test(key)) {
    console.error(`bad seam key '${key}' — expected <module>.<element> kebab-case`)
    process.exit(1)
  }
  const coreFile = resolveInput(coreFileArg)
  if (!fs.existsSync(coreFile)) {
    console.error(`core file not found: ${coreFile}`)
    process.exit(1)
  }
  const { src, changed } = insertAugmentation(fs.readFileSync(coreFile, 'utf-8'), key)
  if (changed) {
    fs.writeFileSync(coreFile, src)
    console.log(`✓ declared seam '${key}' in ${path.relative(REPO, coreFile)}`)
    console.log(`  → now wrap the target element in <Seam id="${key}">…</Seam>`)
  } else {
    console.log(`• seam '${key}' already declared in ${path.relative(REPO, coreFile)}`)
  }
  const stubPath = path.join(OVERRIDES_DIR, `${slugForKey(key)}.tsx`)
  if (fs.existsSync(stubPath)) {
    console.log(`• registration stub already exists: ${path.relative(REPO, stubPath)}`)
  } else {
    fs.mkdirSync(OVERRIDES_DIR, { recursive: true })
    fs.writeFileSync(stubPath, registrationStub(key))
    console.log(`✓ wrote registration stub ${path.relative(REPO, stubPath)}`)
  }
  refreshManifest()
}

function coreSiblingOf(shadowFile) {
  const relFromDesktop = path.relative(DESKTOP_SRC, shadowFile)
  if (relFromDesktop.startsWith('..')) return null
  return path.join(UI_SRC, relFromDesktop)
}

function cmdMigrate(shadowArg) {
  const shadow = resolveInput(shadowArg)
  if (!fs.existsSync(shadow)) {
    console.error(`shadow file not found: ${shadow}`)
    process.exit(1)
  }
  const core = coreSiblingOf(shadow)
  if (!core || !fs.existsSync(core)) {
    console.error(
      `no core sibling for ${path.relative(REPO, shadow)} — a desktop-only file; nothing to migrate.`,
    )
    process.exit(1)
  }
  const cls = classifyDivergence(
    fs.readFileSync(core, 'utf-8'),
    fs.readFileSync(shadow, 'utf-8'),
  )
  console.log(
    `divergence: ${cls.classification} (change ratio ${cls.ratio}, ${cls.changed}/${cls.total} lines)`,
  )
  if (cls.classification === 'structural') {
    console.log(
      `→ STRUCTURAL: keep a tier-1 shadow, or relocate to a co-located ` +
        `\`${path.basename(core).replace(/\.(tsx?)$/, '.desktop.$1')}\` (see UI_OVERRIDES.md). ` +
        `A <Seam> would need many contorted sub-seams — do NOT force one.`,
    )
  } else {
    console.log(
      `→ ELEMENT-LEVEL: a <Seam> fits. Run \`seam-codemod add <module>.<element> ${path.relative(REPO, core)}\`, ` +
        `wrap the diverging element, and move the desktop variant into the generated stub.`,
    )
  }
}

const [, , cmd, ...rest] = process.argv
if (import.meta.url === `file://${process.argv[1]}`) {
  if (cmd === 'add' && rest.length === 2) cmdAdd(rest[0], rest[1])
  else if (cmd === 'migrate' && rest.length === 1) cmdMigrate(rest[0])
  else {
    console.error(
      'usage:\n  seam-codemod add <seam-key> <core-file>\n  seam-codemod migrate <desktop-shadow-file>',
    )
    process.exit(2)
  }
}
