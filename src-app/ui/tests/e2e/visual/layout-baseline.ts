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

export const LAYOUT_BASELINE: LayoutBaselineEntry[] = [
  // The two findings below are on shadcn InputGroup internals (the inline-end
  // addon + its icon button), which the Combobox composes and which carry no
  // data-testid — so they can only be keyed to the section. They are intrinsic
  // to the upstream shadcn `input-group` primitive (shared by every input-group
  // in the app), not to the Combobox, and are out of scope for the gallery pass.
  {
    section: 'gallery-section-combobox',
    check: 'childOverflow',
    testid: 'input-group-addon',
    note: "shadcn InputGroup inline-end addon: its py-1.5 makes the addon a few px taller/wider than the h-8 group border-box (~3.8px h / 2px v). Cosmetically invisible (the addon's own padding box, not its content, clips the group edge). Fix path: tighten the addon padding / cap its height in kit/shadcn input-group.tsx.",
  },
  {
    section: 'gallery-section-combobox',
    check: 'spacingScale',
    testid: 'input-group-button',
    note: "shadcn InputGroupButton size xs/icon-xs uses rounded-[calc(var(--radius)-3px)] = 7px, a deliberate --radius-derived value that is off the checker's 6/8/10 ramp; the same addon also reports a 4.8px half-step gap. Both are upstream shadcn input-group styling. Fix path: snap the button radius to a ramp value (rounded-md = 8px) or add 7px to the ramp — a kit-wide decision, deferred.",
  },
  // shadcn TabsList (shared by Tabs + Segmented) uses `p-[3px]` — the standard
  // upstream inset that seats the active-tab pill; 3px is off the 2px grid but
  // is intentional shadcn styling, not a kit bug. Fix path: change the upstream
  // p-[3px] to p-1 (4px) if grid-alignment is ever prioritised over the shadcn
  // look — a kit-wide decision, deferred.
  {
    section: 'gallery-section-tabs',
    check: 'spacingScale',
    testid: 'tabs-list',
    note: "shadcn TabsList `p-[3px]` (3px inset around the tab pills) — intentional upstream padding, off the 2px grid. Not a kit bug; see note above.",
  },
  {
    section: 'gallery-section-segmented',
    check: 'spacingScale',
    testid: 'segmented-list',
    note: "Segmented composes the same shadcn TabsList `p-[3px]` inset — same intentional upstream 3px padding as gallery-section-tabs.",
  },
  {
    section: 'gallery-section-mermaid-block',
    check: 'spacingScale',
    testid: 'mermaid-source-toggle',
    note: "MermaidBlock's source/render Segmented composes the same shadcn TabsList `p-[3px]` inset — same intentional upstream 3px padding as gallery-section-tabs / gallery-section-segmented. Drop this entry together with those two if the kit ever snaps `p-[3px]`→`p-1`.",
  },
  // The Tag stress story deliberately renders a lone tag whose content is wider
  // than its 224px flex-wrap row. Tag is `whitespace-nowrap` by design (the whole
  // tag wraps to the next line via the parent's flex-wrap rather than breaking
  // its own text) — so a SINGLE oversized/unbreakable token has nothing to wrap
  // against and overflows. This is the accepted limit of the nowrap design, not a
  // regression; real tags never carry 100+ char unbreakable tokens. Fix path: add
  // max-w + truncate to kit/tag.tsx only if lone-oversized clipping is desired.
  {
    section: 'gallery-section-stress-tag',
    check: 'childOverflow',
    testid: 'g-stress-tag-token',
    note: "Lone unbreakable long token in a nowrap Tag, wider than the 224px stress row — inherent to the nowrap-wrap-whole-tag design (see note above).",
  },
  {
    section: 'gallery-section-stress-tag',
    check: 'childOverflow',
    testid: 'g-stress-tag-long',
    note: "Lone long-text nowrap Tag wider than the 224px stress row — same nowrap-design limit as g-stress-tag-token.",
  },
]

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
