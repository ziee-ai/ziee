/**
 * Axe baseline — KNOWN, pre-existing accessibility defects in the kit that the
 * visual-testing system surfaced but that are out of scope for the branch that
 * BUILT the system (it adds the harness; it doesn't fix the kit). The Layer-A
 * a11y pass subtracts these (rule × section) pairs and fails only on anything
 * NEW — so regressions are caught while the documented backlog doesn't keep the
 * gate red.
 *
 * Each entry MUST be a real, triaged finding with a fix path — not a dumping
 * ground. Remove an entry when the underlying kit issue is fixed (the gate will
 * then enforce it). Discovered: feat/shadcn-visual-testing.
 */

export interface AxeBaselineEntry {
  /** axe rule id, e.g. 'color-contrast'. */
  rule: string
  /** gallery section testid the violation lives in, e.g. 'gallery-section-tag'. */
  section: string
  /** Why it's baselined + the fix owner. */
  note: string
}

export const AXE_BASELINE: AxeBaselineEntry[] = [
  // --- ROOT CAUSE: status/tone color tokens use hardcoded Tailwind palette hues
  // (text-green-700, text-amber-600, bg-red-500/10, …) instead of dark-aware
  // AA-tuned semantic tokens (--success/--warning/--error + their fg). This one
  // defect fails WCAG AA contrast (esp. in DARK mode) everywhere a tone-colored
  // kit component renders: Tag, Alert, and the Text success/warning tones — incl.
  // the Tables/scenes that embed status Tags. Fix centrally in the kit's tone
  // token map (kit/tag.tsx, kit/alert.tsx, kit/typography.tsx tone classes), then
  // delete these entries so the gate enforces it. Contradicts DESIGN_DIRECTION
  // "all Tag/Badge tones are WCAG AA". Surfaces enumerated below.
  ...(
    [
      'gallery-section-tag',
      'gallery-section-alert',
      'gallery-section-text',
      'gallery-section-table',
      'gallery-section-scene-table',
    ] as const
  ).map(section => ({
    rule: 'color-contrast',
    section,
    note: 'Pre-existing kit status/tone palette-hue contrast defect (see ROOT CAUSE note in axe-baseline.ts). Fix in the kit tone token map.',
  })),
  {
    rule: 'list',
    section: 'gallery-section-menu',
    note: 'Kit Menu renders <nav><ul> whose direct children are <button>s, not <li>. ul/ol must directly contain li. Fix in components/ui/kit/menu.tsx (wrap each item button in an <li role="none">).',
  },
  {
    rule: 'list',
    section: 'gallery-section-scene-sidebar',
    note: 'Same kit Menu <ul>-without-<li> markup, exercised by the sidebar composite scene. Fixed together with the menu finding above.',
  },
]

/** True when a (rule, section) violation node is a documented, baselined finding. */
export function isBaselined(rule: string, section: string | null): boolean {
  return AXE_BASELINE.some(e => e.rule === rule && e.section === section)
}
