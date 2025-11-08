/**
 * Vite plugin to remove data-test-* attributes from production builds
 *
 * This plugin removes all data-test-* attributes from the final HTML output
 * to reduce bundle size and avoid exposing test identifiers in production.
 *
 * Usage in vite.config.ts:
 * import { removeDataTestPlugin } from './plugins/vite-plugin-remove-data-test.js'
 *
 * plugins: [
 *   // Only remove in production builds
 *   ...(isDev || isTest ? [] : [removeDataTestPlugin()]),
 * ]
 */

export function removeDataTestPlugin() {
  return {
    name: 'remove-data-test-attrs',
    enforce: 'post', // Run after other transformations

    transformIndexHtml: {
      order: 'post',
      handler(html) {
        // Remove data-test-* attributes from HTML
        return html.replace(/\s*data-test-[a-zA-Z0-9-_]*="[^"]*"/g, '')
      }
    },

    transform(code, id) {
      // Only process JS/JSX/TS/TSX files
      if (!/\.(jsx?|tsx?)$/.test(id)) {
        return null
      }

      // Remove data-test-* from createElement calls and JSX
      // This handles both React.createElement and JSX syntax
      let transformed = code

      // Pattern 1: Remove from object properties in createElement/jsx calls
      // e.g., { "data-test-id": "foo", otherProp: "bar" } -> { otherProp: "bar" }
      transformed = transformed.replace(
        /["']data-test-[a-zA-Z0-9-_]*["']\s*:\s*["'][^"']*["']\s*,?\s*/g,
        ''
      )

      // Pattern 2: Remove from JSX attribute syntax
      // e.g., <div data-test-id="foo" /> -> <div />
      transformed = transformed.replace(
        /\s+data-test-[a-zA-Z0-9-_]*=(?:{[^}]*}|"[^"]*"|'[^']*')/g,
        ''
      )

      // Pattern 3: Clean up trailing commas in objects that might result from removal
      transformed = transformed.replace(/,(\s*[}\]])/g, '$1')

      if (transformed !== code) {
        return {
          code: transformed,
          map: null // Don't generate source maps for this simple transformation
        }
      }

      return null
    }
  }
}
