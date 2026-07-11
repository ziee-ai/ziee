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
  // --- Pre-existing on origin/main, NOT introduced by feat/split-chat-multipane ---
  // Proven apples-to-apples: a full runtime-health run on a clean `origin/main`
  // worktree (no split feature) reports the SAME 8 gating HIGH on the SAME 2
  // surfaces (`seeded-llm-models-loading` 6 + `deep-chat-right-panel-file` 2) as
  // this branch. The split diff touches ZERO files in either surface's render
  // subtree (no `llm-provider/` file, no shared kit source; `ChatRightPanel`'s
  // change is inert with inPane=false and adds only an unconditional useEffect).
  // Baselined so the gate reflects that this feature adds no new gating finding;
  // the underlying defects are tracked for their owning modules below.
  {
    category: 'console-error',
    surface: 'seeded-llm-models-loading',
    match: 'order of Hooks',
    note: 'Pre-existing (identical on clean origin/main). `LlmModelsSection` (modules/llm-provider/) trips React "change in the order of Hooks" on the seeded admin models surface — a conditional-hook bug in that admin component, unrelated to split-chat. Owner: llm-provider module. Not in this diff (zero llm-provider files touched).',
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
    note: 'Pre-existing (identical on clean origin/main). The `LlmModelsSection` hook-order defect escalates to an AppErrorBoundary render crash on the seeded admin models surface. Owner: llm-provider module (real bug to fix there). Not in this diff — the split feature touches no llm-provider file.',
  },
  {
    category: 'contrast',
    surface: 'deep-chat-right-panel-file',
    match: 'rgba(0, 0, 0, 0)',
    note: 'Pre-existing (identical on clean origin/main). A transparent foreground (fg rgba(0,0,0,0), alpha 0 — an element mid fade-in / placeholder) computes a degenerate 1.00:1 on the file right-panel surface. Owner: file right-panel component. Not in this diff — `ChatRightPanel`\'s change is inert on this surface (inPane=false) and the file-load path (FileStore) is unchanged.',
  },
  // --- Main-inherited (arrived with the origin/main merge — kb + voice + memory
  // + UI, DRIFT-2.8); NOT introduced by feat/split-chat-multipane. The split diff
  // touches ZERO files in either surface's render subtree. Baselined so the gate
  // reflects that this feature adds no new gating finding; owners noted for the
  // real fixes.
  {
    category: 'console-error',
    surface: 'overlay-provider-api-key-modal',
    match: 'useNavigate() may be used only',
    note: 'Main-inherited gallery-harness limitation: `ProviderApiKeyModal.tsx` (NOT in this diff) calls `useNavigate()` (react-router), and the gallery renders the overlay outside a <Router>, so it throws. The split diff\'s only file in this folder is the sibling `ModelSelector.tsx` (per-pane store binding — no useNavigate/Router change). Owner: gallery overlay harness / user-llm-providers (wrap the overlay in a router context or guard the hook).',
  },
  {
    category: 'crash',
    surface: 'overlay-provider-api-key-modal',
    match: 'useNavigate() may be used only',
    note: 'Same root cause as the paired console-error above — the useNavigate-outside-<Router> throw escalates to an AppErrorBoundary crash on the isolated overlay surface. Main-inherited (`ProviderApiKeyModal.tsx`, not in this diff). Owner: gallery overlay harness / user-llm-providers.',
  },
  {
    category: 'page-error',
    surface: 'settings-memory-admin',
    match: 'Cannot access',
    note: 'Main-inherited: the memory-admin settings surface throws a module circular-init error ("Cannot access \'default\' before initialization") from the memory module\'s own import graph, which arrived with the origin/main merge. The split diff touches ZERO memory-module files. Owner: memory module (break the import cycle).',
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
