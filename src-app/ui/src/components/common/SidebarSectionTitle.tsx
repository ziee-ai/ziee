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
 * Rendered as a real `<h2>`, not a styled `<div>`. Because the caption is a
 * SIBLING of the section's `<nav>` (that is what removes the kit's stacked
 * padding), a div would leave the text stranded outside every landmark on the
 * page: skipped by landmark navigation, and — being neither heading nor
 * labelled region — unreachable by heading-key skimming, which is how a rail
 * is normally scanned. A heading also gives the recent-chats section a name in
 * its error / loading / empty states, where the labelled `<ul>` is not rendered
 * at all and no accessible name would otherwise exist. Tailwind's preflight
 * strips default heading size/margin, so the element renders exactly as before.
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
