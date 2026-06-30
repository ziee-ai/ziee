/**
 * Layout baseline — KNOWN, pre-existing layout defects the stress stories
 * surfaced but that are out of scope for the branch that BUILT the visual system
 * (it adds the harness; it doesn't fix the kit). Mirrors axe-baseline.ts: the
 * Layer-A layout pass subtracts these and fails only on anything NEW.
 *
 * ROOT CAUSE (one defect, many surfaces): kit components don't CONTAIN long
 * unbroken content — they lack `overflow-wrap/break-word`, `min-w-0`, or an
 * ellipsis affordance, so a long token / compound word overflows its container
 * instead of wrapping or truncating. The dedicated `stress-*` sections exist to
 * probe exactly this, so `childOverflow` + `textTruncation` there are baselined
 * as that systemic finding. Confirmed surfaces: Tag (long token), Select trigger
 * (no ellipsis on long label), Card title/header, Menu items, Descriptions
 * values. Fix centrally (add break-words/min-w-0/truncate to the affected kit
 * components), then delete this rule so the gate enforces containment.
 *
 * Every OTHER check (spacing, overlap, button-width, …) stays LIVE on the stress
 * sections, and ALL checks stay live on non-stress sections. Discovered:
 * feat/shadcn-visual-testing.
 */
import type { LayoutCheck } from '../helpers/layout'

const STRESS_PREFIX = 'gallery-section-stress-'
const STRESS_CONTENT_CHECKS: LayoutCheck[] = ['childOverflow', 'textTruncation']

export interface LayoutBaselineEntry {
  section: string
  check: LayoutCheck
  note: string
}

/**
 * Explicit, non-stress baselined findings (kept enumerated for visibility).
 * Currently empty — the only documented layout findings are the systemic
 * long-content overflow on the stress sections, handled by the rule below.
 */
export const LAYOUT_BASELINE: LayoutBaselineEntry[] = []

export function isLayoutBaselined(
  section: string | null,
  check: LayoutCheck,
): boolean {
  if (
    section?.startsWith(STRESS_PREFIX) &&
    STRESS_CONTENT_CHECKS.includes(check)
  ) {
    return true
  }
  return LAYOUT_BASELINE.some(e => e.section === section && e.check === check)
}
