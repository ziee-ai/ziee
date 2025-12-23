/**
 * Vite Plugin: Local Override
 *
 * Automatically overrides core UI files when desktop has a file at the same path.
 *
 * When resolving `@/some/path`:
 * - If `desktop/ui/src/some/path.ts` (or .tsx) exists → use desktop's version
 * - Otherwise → use core UI's version
 */

import type { Plugin } from 'vite'
import path from 'path'
import fs from 'fs'

interface LocalOverrideOptions {
  /** Local src directory to check first */
  localSrc: string
  /** Fallback src directory (core UI) */
  fallbackSrc: string
  /** Alias prefix to handle (e.g., '@/') */
  aliasPrefix: string
}

export function localOverridePlugin(options: LocalOverrideOptions): Plugin {
  const { localSrc, fallbackSrc, aliasPrefix } = options
  const extensions = ['.ts', '.tsx', '.js', '.jsx', '.json', '.css']

  return {
    name: 'vite-plugin-local-override',
    enforce: 'pre',

    resolveId(source) {
      // Only handle imports starting with alias prefix
      if (!source.startsWith(aliasPrefix)) {
        return null
      }

      // Get relative path after alias
      const relativePath = source.slice(aliasPrefix.length)

      // Check if file exists in local src (with various extensions)
      for (const ext of ['', ...extensions]) {
        const localPath = path.join(localSrc, relativePath + ext)
        if (fs.existsSync(localPath)) {
          // Also check it's a file, not directory
          if (fs.statSync(localPath).isFile()) {
            return localPath
          }
        }
        // Check for index file in directory
        if (ext === '') {
          for (const indexExt of extensions) {
            const indexPath = path.join(localSrc, relativePath, `index${indexExt}`)
            if (fs.existsSync(indexPath) && fs.statSync(indexPath).isFile()) {
              return indexPath
            }
          }
        }
      }

      // Fall back to core UI
      for (const ext of ['', ...extensions]) {
        const fallbackPath = path.join(fallbackSrc, relativePath + ext)
        if (fs.existsSync(fallbackPath)) {
          if (fs.statSync(fallbackPath).isFile()) {
            return fallbackPath
          }
        }
        // Check for index file in directory
        if (ext === '') {
          for (const indexExt of extensions) {
            const indexPath = path.join(fallbackSrc, relativePath, `index${indexExt}`)
            if (fs.existsSync(indexPath) && fs.statSync(indexPath).isFile()) {
              return indexPath
            }
          }
        }
      }

      return null // Let other resolvers handle it
    },
  }
}
