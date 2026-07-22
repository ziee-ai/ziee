interface SidebarSectionTitleProps {
  /** Caption text, e.g. "Navigation" / "Tools" / "Recent chats". */
  children: React.ReactNode
  /** Test selector — the sidebar renders several of these, so each names itself. */
  'data-testid'?: string
}

/**
 * The left sidebar's section caption — ONE source of truth for the inset and the
 * typography of "Navigation", "Tools" and "Recent chats".
 *
 * Before this existed the three captions were produced two different ways and
 * landed on two different left edges: "Navigation"/"Tools" were kit `Menu` GROUP
 * titles, whose inset is the Menu wrapper's `px-2` (8px) PLUS the kit's own
 * hardcoded group-title `px-3` (12px) = 20px, while "Recent chats" was a
 * hand-rolled div at a flat `px-3` = 12px. They had also drifted apart in weight
 * and tracking, despite a comment claiming one "mirrors" the other.
 *
 * The caption deliberately sits 8px LEFT of the menu rows below it (rows are
 * `px-2` + `px-3` = 20px): the captions hang outside their rows so the section
 * headings form their own vertical scan line down the rail. That is the
 * relationship "Recent chats" already had, now applied consistently.
 *
 * Deliberately a `<div>` and NOT a heading, even though the caption is now a
 * SIBLING of its section's `<nav>` rather than inside it (that sibling
 * placement is what removes the kit's stacked padding).
 *
 * A heading was tried and reverted. It does make the caption reachable by
 * heading-key skimming, but this app's page titles are `Title level={4}`
 * (~15 call sites) and there is effectively no `<h1>`, so putting three `<h2>`s
 * in the sidebar — which the shell renders BEFORE `<main>` on every page —
 * produces an h2, h2, h2, h4 sequence with a skipped level, app-wide. Trading a
 * stranded-caption problem confined to the rail for a broken heading outline on
 * every page is a bad deal, and fixing it properly means settling the app's
 * heading hierarchy, which is well outside an alignment fix.
 *
 * Known, deferred consequence: "Navigation" and "Tools" no longer sit inside
 * the `<nav aria-label=…>` they label, so landmark navigation reaches the menus
 * without their visible caption. The landmarks are still named by their own
 * `aria-label`, so no section is unnamed. Neither state is caught by a gate —
 * the axe pass runs `wcag2a`/`wcag2aa` only (excluding the best-practice
 * `region` and `heading-order` rules) and the gallery never renders the
 * sidebar — so both readings here are reasoned from the DOM, not measured.
 */
export function SidebarSectionTitle({
  children,
  'data-testid': testid,
}: SidebarSectionTitleProps) {
  return (
    <div
      data-testid={testid}
      className="px-3 pt-0 pb-1 text-xs font-semibold tracking-wide text-muted-foreground"
    >
      {children}
    </div>
  )
}
