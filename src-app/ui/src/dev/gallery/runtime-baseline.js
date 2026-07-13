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
  // ── Pre-existing product-component findings (NOT introduced by the
  //    modular-seed feature). Verified to fail IDENTICALLY on origin/main
  //    (e2b5bba) — these surfaces are migrated VERBATIM and render through
  //    unchanged frames, so the defect is in the product component, not the
  //    gallery seed. Scoped by `match` so a DIFFERENT crash on the same surface
  //    still gates. Owner: the respective module.
  // overlay-provider-api-key-modal: useNavigate outside a Router — the crash AND
  // its console-error twin (React logs the boundary-caught error to console).
  {
    category: 'crash',
    surface: 'overlay-provider-api-key-modal',
    match: 'useNavigate() may be used only in the context of a <Router>',
    note: 'ProviderApiKeyModal (user-llm-providers) calls useNavigate but the gallery OverlayFrame renders it without a Router context. Pre-existing (fails identically on origin/main e2b5bba). Owner: user-llm-providers — drop the useNavigate or wrap the modal in a router at its call site.',
  },
  {
    category: 'console-error',
    surface: 'overlay-provider-api-key-modal',
    match: 'useNavigate() may be used only',
    note: 'Console-error twin of the useNavigate crash above (React logs the boundary-caught error). Pre-existing (origin/main e2b5bba).',
  },
  // seeded-llm-models-loading: Rules-of-Hooks violation — the crash + its two
  // console-error twins (the throw + React\'s hook-order-change warning).
  {
    category: 'crash',
    surface: 'seeded-llm-models-loading',
    match: 'Rendered more hooks than during the previous render',
    note: 'The llm-models list component varies its hook count between the loading and loaded renders (a Rules-of-Hooks violation surfaced by the loading-state seed). Pre-existing (fails identically on origin/main e2b5bba). Owner: llm-provider.',
  },
  {
    category: 'console-error',
    surface: 'seeded-llm-models-loading',
    match: 'Rendered more hooks than during the previous render',
    note: 'Console-error twin of the hooks-count crash above. Pre-existing (origin/main e2b5bba).',
  },
  {
    category: 'console-error',
    surface: 'seeded-llm-models-loading',
    match: 'change in the order of Hooks',
    note: 'React\'s hook-order-change warning — same Rules-of-Hooks root cause as the crash above. Pre-existing (origin/main e2b5bba).',
  },
  {
    category: 'console-error',
    surface: 'seeded-s3-group-widget-error',
    match: '/api/groups/',
    note: 'This surface DELIBERATELY installs a one-time window.fetch shim that 500s GET /api/groups/:id/providers to exercise the LLMProviderGroupWidget error state — the console-error is the intended, seeded failure. Pre-existing (fails identically on origin/main e2b5bba).',
  },
  {
    category: 'contrast',
    surface: 'deep-chat-right-panel-file',
    match: 'rgba(0, 0, 0, 0)',
    note: 'A transparent-foreground (alpha-0) text node computes a 1.00:1 ratio on the chat right-panel file view (an empty/placeholder text span). Pre-existing (fails identically on origin/main e2b5bba). Owner: chat right-panel.',
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
