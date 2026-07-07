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
 *    from both trees and a cross-tree collision is real. A desktop file that
 *    OVERRIDES a core file at the same `@/...` path is de-duplicated by
 *    relative path so the override (not a phantom collision) is what counts.
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
const TESTID_LITERAL = /data-testid\s*[=:]\s*["']([^"']+)["']/g

// The detector-acceptance fixture INTENTIONALLY reuses the real app testids the
// detectors key on (e.g. K1 matches the literal `conversation-title`, so its
// known-bad repro cell must carry exactly that id) — so it is exempt from the
// cross-file uniqueness gate. It never renders in the real app.
const TESTID_EXEMPT = /[/\\]dev[/\\]gallery[/\\]DefectRepro\.tsx$/

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

/**
 * Scan one-or-more source roots, deduping override files by their `@/...`
 * relative path (desktop overrides shadow core), and return a Map<id, file[]>.
 */
function collectTestids(srcDirs) {
  // relPath -> { file, root } : a later root in the list wins (desktop override
  // shadows core), so we walk roots in order and the LAST writer keeps the slot.
  const byRel = new Map()
  for (const root of srcDirs) {
    for (const file of findSourceFiles(root)) {
      const rel = path.relative(root, file)
      byRel.set(rel, file)
    }
  }
  const idMap = new Map() // id -> Set<relPath> (deduped per file)
  for (const [rel, file] of byRel.entries()) {
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
    // design); a collision is the SAME id claimed by two DIFFERENT files.
    if (fileSet.size > 1) duplicates.push({ id, files: [...fileSet] })
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
