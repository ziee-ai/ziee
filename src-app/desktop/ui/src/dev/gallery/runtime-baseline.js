/**
 * Runtime-health baseline — KNOWN, pre-existing findings the runtime-health pass
 * surfaces but that are out of scope for a mechanical fix (they need a design
 * decision, e.g. a color-token change that ripples app-wide). The gate SUBTRACTS
 * these before computing its HIGH gating count, so it stays green on the current
 * tree while still failing on anything NEW or WORSE.
 *
 * This is the runtime analog of `tests/e2e/visual/axe-baseline.ts` /
 * `layout-baseline.ts`. Keep each entry as narrow as possible (surface + token
 * signature) so it can't mask a different finding on the same surface. Every
 * entry MUST carry a `note` (why it's baselined + the fix owner). Do NOT use this
 * as a dumping ground — a genuinely new crash / contrast regression should fail
 * the gate, not get parked here.
 *
 * Plain JS (not TS) so `scripts/runtime-health.mjs` can `import` it directly
 * without a transpile step.
 */

/**
 * @typedef {Object} RuntimeBaselineEntry
 * @property {string} category   finding category, e.g. 'contrast'
 * @property {string} surface    gallery surface slug, e.g. 'onboarding'
 * @property {string} [match]    substring that must appear in the finding detail
 *                               (e.g. the foreground color token) — scopes the
 *                               baseline to ONE token, not the whole surface.
 * @property {string} note       why it's baselined + who owns the real fix.
 */

/** @type {RuntimeBaselineEntry[]} */
export const RUNTIME_BASELINE = [
  {
    category: 'contrast',
    surface: 'hub',
    match: 'oklch(0.577 0.245 27.325)',
    note: 'Destructive/red token (--destructive) as 12px text on the light-theme destructive alert tint (bg ~rgb(252,229,230)) computes to 3.97:1, just under AA 4.5:1. This is the destructive-on-destructive-surface combo; darkening --destructive or the alert tint is an app-wide design-token decision (owner: design). Large/heading uses of the same token pass (AA-large 3:1).',
  },
  {
    category: 'contrast',
    surface: 'onboarding',
    match: 'oklch(0.556 0 0)',
    note: 'Muted-foreground token (--muted-foreground) as 12px text on the light muted surface (bg ~rgb(245,245,245)) computes to 4.35:1, marginally under AA 4.5:1. Raising --muted-foreground contrast is an app-wide token decision (owner: design); appears across onboarding loaded/empty/error (same token, same surface).',
  },
  // ── Pre-existing on origin/main (NOT the split-chat-multipane diff) ──────────
  // Mirrors the web (ui) baseline: the desktop gallery scans the SAME shared web
  // source, so it renders the same surfaces + the same pre-existing findings.
  // Verified identical on a clean origin/main worktree; the split diff touches
  // neither surface's render subtree. See src-app/ui/src/dev/gallery/runtime-baseline.js.
  {
    category: 'console-error',
    surface: 'seeded-llm-models-loading',
    match: 'order of Hooks',
    note: 'Pre-existing (identical on clean origin/main). `LlmModelsSection` (modules/llm-provider/) trips React "change in the order of Hooks" on the seeded admin models surface — a conditional-hook bug in that admin component, unrelated to split-chat. Owner: llm-provider module. Not in this diff.',
  },
  {
    category: 'console-error',
    surface: 'seeded-llm-models-loading',
    match: 'Rendered more hooks',
    note: 'Pre-existing (identical on clean origin/main). Same `LlmModelsSection` conditional-hook defect — "Rendered more hooks than during the previous render". Owner: llm-provider module. Not in this diff.',
  },
  {
    category: 'crash',
    surface: 'seeded-llm-models-loading',
    match: 'Rendered more hooks',
    note: 'Pre-existing (identical on clean origin/main). The `LlmModelsSection` hook-order defect escalates to an AppErrorBoundary render crash on the seeded admin models surface. Owner: llm-provider module. Not in this diff.',
  },
  {
    category: 'contrast',
    surface: 'deep-chat-right-panel-file',
    match: 'rgba(0, 0, 0, 0)',
    note: 'Pre-existing (identical on clean origin/main). A transparent foreground (fg rgba(0,0,0,0), alpha 0) computes a degenerate 1.00:1 on the file right-panel surface. Owner: file right-panel component. Not in this diff.',
  },
]

/** True when a finding is a documented, baselined pre-existing item. */
export function isRuntimeBaselined(finding) {
  return RUNTIME_BASELINE.some(
    e =>
      e.category === finding.category &&
      e.surface === finding.surface &&
      (e.match == null || (finding.detail ?? '').includes(e.match)),
  )
}
