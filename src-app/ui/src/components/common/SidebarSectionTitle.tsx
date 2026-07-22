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
 * Rendered as a real `<h2>`, not a styled `<div>`, and that is a deliberate
 * mitigation rather than a free win. Removing the kit's stacked padding means
 * the caption is now a SIBLING of the section's `<nav>` instead of living
 * inside it, so the text sits outside every landmark on the page — the kit used
 * to render it within the `<nav aria-label=…>` it names. A heading does not put
 * it back inside that landmark; it gives the stranded text a role, so it is
 * still reachable by heading-key skimming, which is how a rail is actually
 * scanned. It also gives the recent-chats section a name in its error /
 * loading / empty states, where the labelled `<ul>` is not rendered at all and
 * no accessible name would otherwise exist.
 *
 * Tailwind's preflight zeroes heading margin and sets `font-size`/`font-weight`
 * to `inherit`, so the element renders pixel-identically to the previous div.
 *
 * Not verified by a gate: the axe pass runs `wcag2a`/`wcag2aa` only (so the
 * best-practice `region` rule is off) and the gallery never renders the
 * sidebar, so this is reasoned from the DOM rather than measured.
 */
export function SidebarSectionTitle({
  children,
  'data-testid': testid,
}: SidebarSectionTitleProps) {
  return (
    <h2
      data-testid={testid}
      className="px-3 pt-0 pb-1 text-xs font-semibold tracking-wide text-muted-foreground"
    >
      {children}
    </h2>
  )
}
