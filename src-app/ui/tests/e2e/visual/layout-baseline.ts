/**
 * Layout baseline — KNOWN, pre-existing layout defects the visual system caught
 * but that are out of scope to fix. Mirrors axe-baseline.ts: the Layer-A layout
 * pass subtracts these and fails only on anything NEW.
 *
 * Currently EMPTY: the long-unbroken-content overflow the stress stories
 * surfaced has been FIXED in the kit (Tag, Card title, Menu item, Descriptions
 * value now contain/wrap/truncate), and the "Select clips without ellipsis"
 * finding was a CHECKER false-negative (line-clamp wasn't recognized as an
 * ellipsis affordance) — fixed in layout.ts. So the gate now enforces
 * containment with no baseline.
 *
 * If you must re-add an entry, key it to the specific element (testid) — NOT a
 * whole section — so a NEW overflow on a different element in that section still
 * fails. Each entry needs a real fix path.
 */
import type { LayoutCheck } from '../helpers/layout'

export interface LayoutBaselineEntry {
  section: string
  check: LayoutCheck
  /** Element testid the finding is on (element-scoped, not section-wide). */
  testid: string
  note: string
}

export const LAYOUT_BASELINE: LayoutBaselineEntry[] = []

export function isLayoutBaselined(
  section: string | null,
  check: LayoutCheck,
  testid?: string | null,
): boolean {
  return LAYOUT_BASELINE.some(
    e =>
      e.section === section &&
      e.check === check &&
      (testid == null || e.testid === testid),
  )
}
