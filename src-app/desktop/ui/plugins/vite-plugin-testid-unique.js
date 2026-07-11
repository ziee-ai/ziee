/**
 * Vite plugin: enforce GLOBALLY-UNIQUE `data-testid` literals.
 *
 * Tests must survive i18n, so visible-text / label / role-name selectors are
 * untrustworthy — `data-testid` is the primary selector. The kit makes a
 * testid REQUIRED (tsc-required prop) on every functional/container component,
 * so PRESENCE is guaranteed at compile time. This plugin guarantees the other
 * half — UNIQUENESS — at build time: it scans the source for static
 * `data-testid="literal"` occurrences and FAILS the build on any duplicate.
 *
 * Scope:
 *  - ui build:      scan `src-app/ui/src`.
 *  - desktop build: scan BOTH `src-app/desktop/ui/src` AND `src-app/ui/src` —
 *    the desktop app renders core-ui source via the localOverridePlugin
 *    (desktop-first, core-ui fallback), so a desktop bundle contains literals
 *    from both trees and a cross-tree collision is real. A file that OVERRIDES
 *    a core file — either a desktop-tree file at the same `@/...` path (tier 1)
 *    or a co-located core-tree `<path>.desktop.<ext>` (tier 2) — is collapsed
 *    onto the base's import slot (see `collectTestids`) so the override (not a
 *    phantom collision) is what counts.
 *
 * Exemptions:
 *  - Template-literal / expression testids (`data-testid={`${x}-row-${k}`}`)
 *    are DERIVED from a required (and therefore unique) container testid, so
 *    their scoping already guarantees uniqueness — only STATIC double/single
 *    quoted literals are dup-checked here.
 *  - The SAME literal repeated WITHIN ONE FILE is allowed: that's the
 *    conditional-branch / loop pattern (mutually-exclusive `cond ? <A id="x"/>
 *    : <B id="x"/>`), where one logical element shares one testid by design —
 *    a test selecting `x` wants it regardless of which branch rendered. The
 *    gate is therefore CROSS-FILE uniqueness: an id must not live in two files.
 */

import fs from 'node:fs'
import path from 'node:path'

// Matches data-testid="literal" / data-testid='literal' (JSX) and the object
// form "data-testid": "literal" (spread props). NOT data-testid={expr}.
// `(?<!\[)` skips CSS attribute SELECTORS — `querySelector('[data-testid="x"]')`
// READS a testid, it does not DECLARE one, so it must not count as a second
// declaration (that false-positived `kb-tool-result-*` as duplicates and broke
// the desktop `vite build` — a PRE-EXISTING bug, unrelated to UI overrides).
const TESTID_LITERAL = /(?<!\[)data-testid\s*[=:]\s*["']([^"']+)["']/g

// The detector-acceptance fixture INTENTIONALLY reuses the real app testids the
// detectors key on (e.g. K1 matches the literal `conversation-title`, so its
// known-bad repro cell must carry exactly that id) — so it is exempt from the
// cross-file uniqueness gate. It never renders in the real app. The desktop
// build scans the shared web tree (fallback alias), which contains
// `dev/gallery/DefectRepro.tsx`, so it needs the SAME exemption the web plugin
// has (kept in parity — the two plugins scan the same web source).
const TESTID_EXEMPT = /[/\\]dev[/\\]gallery[/\\]DefectRepro\.tsx$/

// Intentionally-shared testids: the SAME logical control rendered in two
// MUTUALLY-EXCLUSIVE modes (never both mounted at once), which e2e selects
// mode-agnostically. The elicitation submit/decline/form/pending controls appear
// in BOTH the multi-step wizard (`AskUserWizardContent`) and the single-form
// (`ElicitationFormContent`) renderer of one elicitation; the chat e2e specs
// (07/09-chat/*elicitation*) select these regardless of which mode renders, so
// they must stay identical across the two files.
const ALLOWED_SHARED_TESTIDS = new Set([
  'elicitation-decline',
  'elicitation-submit',
  'mcp-elicitation-form',
  'mcp-elicitation-pending-card',
])

function findSourceFiles(dir, fileList = []) {
  if (!fs.existsSync(dir)) return fileList
  for (const entry of fs.readdirSync(dir)) {
    const full = path.join(dir, entry)
    const stat = fs.statSync(full)
    if (stat.isDirectory()) {
      if (!['node_modules', 'dist', 'build', '.git', 'tests'].includes(entry)) {
        findSourceFiles(full, fileList)
      }
    } else if (/\.(tsx|jsx|ts)$/.test(entry) && !TESTID_EXEMPT.test(full)) {
      fileList.push(full)
    }
  }
  return fileList
}

function extractTestids(content) {
  const ids = []
  let m
  TESTID_LITERAL.lastIndex = 0
  while ((m = TESTID_LITERAL.exec(content)) !== null) {
    const id = m[1]
    if (id && id.trim()) ids.push(id.trim())
  }
  return ids
}

// A co-located `Foo.desktop.tsx` is the localOverridePlugin's tier-2 shadow of
// its `Foo.tsx` sibling: in the desktop bundle the `@/…/Foo` import resolves to
// the `.desktop` file and the base file is NOT bundled at that path. So the two
// files occupy ONE import slot and must NOT collide on a shared testid — a
// whole-file override legitimately keeps the SAME data-testid as the core file
// it replaces so e2e selects it identically across web and desktop.
const DESKTOP_SHADOW = /\.desktop\.(tsx|jsx|ts)$/

/**
 * Scan one-or-more source roots and resolve every file to the single import
 * "slot" it occupies, so a testid shared between a base file and the file that
 * SHADOWS it is counted once (not flagged as a false cross-file duplicate).
 * Returns Map<id, Set<relPath>>.
 *
 * Two shadow mechanisms collapse to one slot, mirroring the localOverridePlugin
 * resolver's precedence (highest wins):
 *   - tier 1: a desktop-tree file (`desktop/ui/src/<path>.<ext>`) shadows the
 *     core-tree file at the same relative path — expressed here as "a LATER
 *     srcDir outranks an earlier one".
 *   - tier 2: a co-located core-tree `<path>.desktop.<ext>` shadows its
 *     `<path>.<ext>` base — expressed here as "within one srcDir a `.desktop.*`
 *     variant outranks its plain base sibling" (both normalize to the base slot).
 */
function collectTestids(srcDirs) {
  // slot(relPathWithoutDesktopInfix) -> { rel, file, rank } : the highest-rank
  // writer keeps the slot. rank = rootIndex*2 + (isDesktopShadow ? 1 : 0), a
  // total order that reproduces the resolver precedence above.
  const bySlot = new Map()
  srcDirs.forEach((root, rootIndex) => {
    for (const file of findSourceFiles(root)) {
      const rel = path.relative(root, file)
      const isDesktopShadow = DESKTOP_SHADOW.test(rel)
      // `Foo.desktop.tsx` → `Foo.tsx`: collapse a tier-2 shadow onto its base's
      // slot so the two share one slot instead of two distinct rel-path keys.
      const slot = isDesktopShadow ? rel.replace(DESKTOP_SHADOW, '.$1') : rel
      const rank = rootIndex * 2 + (isDesktopShadow ? 1 : 0)
      const existing = bySlot.get(slot)
      if (!existing || rank >= existing.rank) {
        bySlot.set(slot, { rel, file, rank })
      }
    }
  })
  const idMap = new Map() // id -> Set<relPath> (deduped per surviving slot file)
  for (const { rel, file } of bySlot.values()) {
    const ids = extractTestids(fs.readFileSync(file, 'utf-8'))
    for (const id of ids) {
      if (!idMap.has(id)) idMap.set(id, new Set())
      idMap.get(id).add(rel)
    }
  }
  return idMap
}

function checkUnique(srcDirs, logger) {
  const idMap = collectTestids(srcDirs)
  const duplicates = []
  for (const [id, fileSet] of idMap.entries()) {
    // Allowed within a single file (conditional branches share one testid by
    // design); a collision is the SAME id claimed by two DIFFERENT files —
    // EXCEPT the explicitly allow-listed intentionally-shared ids.
    if (fileSet.size > 1 && !ALLOWED_SHARED_TESTIDS.has(id))
      duplicates.push({ id, files: [...fileSet] })
  }
  if (duplicates.length > 0) {
    let msg = `\n[testid-unique] ✗ Found ${duplicates.length} duplicate data-testid literal(s):\n`
    for (const { id, files } of duplicates) {
      msg += `  • "${id}"\n`
      for (const f of files) msg += `      - ${f}\n`
    }
    msg += '\ndata-testid must be globally unique (tests select by it under i18n).\n'
    logger.error(msg)
    throw new Error(`[testid-unique] ${duplicates.length} duplicate data-testid literal(s) — see log above.`)
  }
  logger.info(`[testid-unique] ✓ All data-testid literals unique (${idMap.size} ids).`)
  return { total: idMap.size }
}

/**
 * @param {{ srcDirs: string[] }} options absolute source roots to scan, in
 *   override order (earliest = lowest priority, latest shadows).
 */
export function testidUniquePlugin(options = {}) {
  const { srcDirs = [] } = options
  let logger
  let debounceTimer

  return {
    name: 'vite-plugin-testid-unique',

    configResolved(config) {
      logger = config.logger
    },

    buildStart() {
      checkUnique(srcDirs, logger)
    },

    handleHotUpdate({ file }) {
      if (/\.(tsx|jsx|ts)$/.test(file) && !file.includes('node_modules')) {
        clearTimeout(debounceTimer)
        debounceTimer = setTimeout(() => {
          try {
            checkUnique(srcDirs, logger)
          } catch (e) {
            // In dev, warn but don't crash the server; the build gate enforces.
            logger.warn(String(e.message || e))
          }
        }, 1000)
      }
    },
  }
}
