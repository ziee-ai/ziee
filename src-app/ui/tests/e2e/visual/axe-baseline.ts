/**
 * Axe baseline — KNOWN, pre-existing accessibility defects in the kit that the
 * visual-testing system surfaced but that are out of scope to fix. The Layer-A
 * a11y pass subtracts these and fails only on anything NEW.
 *
 * Currently EMPTY: every finding the system originally caught (status/tone color
 * contrast in dark mode; Menu `<ul>` containing non-`<li>` children; Table
 * scroll-region not keyboard-focusable) has been FIXED in the kit, so the gate
 * now enforces them with no baseline. Re-add an entry (keyed as narrowly as
 * possible — ideally by node target, not whole section, so it can't mask a NEW
 * violation on a different element) only for a real, triaged finding with a fix
 * path. Do not use this as a dumping ground.
 */

export interface AxeBaselineEntry {
  /** axe rule id, e.g. 'color-contrast'. */
  rule: string
  /** gallery section testid the violation lives in, e.g. 'gallery-section-tag'. */
  section: string
  /** Optional CSS-target substring — when set, ONLY nodes whose axe target
   *  matches are baselined (keeps the baseline from masking new violations on
   *  other elements in the same section). */
  targetIncludes?: string
  /** Why it's baselined + the fix owner. */
  note: string
}

export const AXE_BASELINE: AxeBaselineEntry[] = []

/**
 * True when a violation is a documented, baselined finding. Matches on
 * (rule, section) and — when the entry specifies `targetIncludes` — also requires
 * the node's selector to contain that substring, so the baseline is element-scoped
 * rather than swallowing the whole section.
 */
export function isBaselined(
  rule: string,
  section: string | null,
  target?: string,
): boolean {
  return AXE_BASELINE.some(
    e =>
      e.rule === rule &&
      e.section === section &&
      (e.targetIncludes == null || (target ?? '').includes(e.targetIncludes)),
  )
}
