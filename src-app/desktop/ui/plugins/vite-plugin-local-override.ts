/**
 * Vite Plugin: Local Override
 *
 * Resolves `@/…` imports for the desktop build across three tiers (highest
 * precedence first — see DEC-14 in the desktop-ui-override lifecycle):
 *
 *   1. desktop-tree shadow   `desktop/ui/src/<path>.<ext>`       (whole-file override, unchanged)
 *   2. core-tree `.desktop.` `ui/src/<path>.desktop.<ext>`        (co-located whole-file override, NEW)
 *   3. core base             `ui/src/<path>.<ext>`                (the shared web implementation)
 *
 * Tier 1 keeps the historical desktop-tree mirror working (desktop-only modules,
 * anything not yet relocated). Tier 2 lets a whole-file override live NEXT TO its
 * core sibling as `Foo.desktop.tsx` — the web build never imports it (nothing
 * references a `.desktop` specifier) and the web workspace excludes `**/*.desktop.*`
 * from tsc/biome, so the Tauri-importing file is inert there.
 *
 * The resolution ORDER is a pure function (`resolveOverridePath`) so it can be
 * unit-tested without a Vite dev server.
 */

import type { Plugin } from 'vite'
import path from 'path'
import fs from 'fs'

export interface LocalOverrideOptions {
  /** Local src directory to check first (desktop tree). */
  localSrc: string
  /** Fallback src directory (core UI). */
  fallbackSrc: string
  /** Alias prefix to handle (e.g., '@/'). */
  aliasPrefix: string
}

const EXTENSIONS = ['.ts', '.tsx', '.js', '.jsx', '.json', '.css']

function fileAt(candidate: string): string | null {
  return fs.existsSync(candidate) && fs.statSync(candidate).isFile()
    ? candidate
    : null
}

/** Probe `dir/relative(.ext)` as a file, then `dir/relative/index.<ext>`. */
function probeFileOrIndex(dir: string, relative: string): string | null {
  for (const ext of ['', ...EXTENSIONS]) {
    const hit = fileAt(path.join(dir, relative + ext))
    if (hit) return hit
  }
  for (const ext of EXTENSIONS) {
    const hit = fileAt(path.join(dir, relative, `index${ext}`))
    if (hit) return hit
  }
  return null
}

/** Probe the co-located `dir/relative.desktop.<ext>` (file form only). */
function probeDesktopInfix(dir: string, relative: string): string | null {
  for (const ext of EXTENSIONS) {
    const hit = fileAt(path.join(dir, `${relative}.desktop${ext}`))
    if (hit) return hit
  }
  return null
}

/**
 * Pure resolution of an `@/…` import to a concrete file path across the three
 * tiers, or `null` when the prefix doesn't match or nothing exists (let other
 * resolvers handle it). Exported for unit testing.
 */
export function resolveOverridePath(
  source: string,
  options: LocalOverrideOptions,
): string | null {
  const { localSrc, fallbackSrc, aliasPrefix } = options
  if (!source.startsWith(aliasPrefix)) return null
  const relative = source.slice(aliasPrefix.length)

  // Tier 1: desktop-tree shadow (highest precedence, historical behavior).
  const desktop = probeFileOrIndex(localSrc, relative)
  if (desktop) return desktop

  // Tier 2: core-tree co-located `.desktop.*` override (NEW).
  const coreDesktop = probeDesktopInfix(fallbackSrc, relative)
  if (coreDesktop) return coreDesktop

  // Tier 3: core base — the shared web implementation.
  const core = probeFileOrIndex(fallbackSrc, relative)
  if (core) return core

  return null // Let other resolvers handle it.
}

export function localOverridePlugin(options: LocalOverrideOptions): Plugin {
  return {
    name: 'vite-plugin-local-override',
    enforce: 'pre',

    resolveId(source) {
      return resolveOverridePath(source, options)
    },
  }
}
