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
 * This is decorative section chrome, not a heading landmark — each sidebar
 * section is already named for assistive tech by its `<nav aria-label=…>`
 * (or, for recent chats, the list's own `aria-label`), so it is intentionally
 * NOT an `<h*>`: adding one would announce a heading that duplicates the
 * landmark name.
 */
export function SidebarSectionTitle({
  children,
  'data-testid': testid,
}: SidebarSectionTitleProps) {
  return (
    <div
      data-testid={testid}
      className="px-3 pt-0 pb-0.5 text-xs font-semibold tracking-wide text-muted-foreground"
    >
      {children}
    </div>
  )
}
