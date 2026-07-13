import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

// TEST-7 (covers ITEM-5): the `--auth-backdrop` token — which drives the
// unauthenticated-screen backdrop edge color AND the iOS meta[theme-color] — must
// be declared in BOTH the light (`:root`) and dark (`.dark`) scopes so the
// screen-edge color follows the theme. Deterministic complement to the runtime
// meta-color check (TEST-5) and to `check:design-spec`.

const css = readFileSync(
  fileURLToPath(new URL('./index.css', import.meta.url)),
  'utf-8',
)

// extract a top-level `selector { ... }` ruleset body (brace-balanced). Anchors
// on the selector IMMEDIATELY followed by `{` so a bare mention (e.g. the
// `@custom-variant dark (&:is(.dark *))` line) is not mistaken for the ruleset.
function block(selector: string): string {
  const esc = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const m = new RegExp(`${esc}\\s*\\{`).exec(css)
  assert.ok(m, `ruleset ${selector} {…} not found in index.css`)
  const open = css.indexOf('{', m.index)
  let depth = 0
  for (let j = open; j < css.length; j++) {
    if (css[j] === '{') depth++
    else if (css[j] === '}' && --depth === 0) return css.slice(open + 1, j)
  }
  throw new Error(`unterminated block for ${selector}`)
}

test('--auth-backdrop is declared in the light (:root) scope', () => {
  assert.match(block(':root'), /--auth-backdrop\s*:\s*[^;]+;/)
})

test('--auth-backdrop is declared in the dark (.dark) scope', () => {
  assert.match(block('.dark'), /--auth-backdrop\s*:\s*[^;]+;/)
})

test('the two --auth-backdrop values differ (light ≠ dark → follows the theme)', () => {
  const light = /--auth-backdrop\s*:\s*([^;]+);/.exec(block(':root'))?.[1].trim()
  const dark = /--auth-backdrop\s*:\s*([^;]+);/.exec(block('.dark'))?.[1].trim()
  assert.ok(light && dark, 'both values present')
  assert.notEqual(light, dark)
})
