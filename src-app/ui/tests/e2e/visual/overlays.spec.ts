/**
 * Overlay OPEN-state coverage — the blind spot the audit flagged: the gallery
 * stories render only a closed trigger, and overlay content is portaled to
 * <body> (outside any gallery-section), so the per-section screenshots never
 * capture the actual dialog/sheet/popover/dropdown/menu. This spec opens each
 * overlay (Storybook-play style), then:
 *   - runs the Layer-A layout invariants on the open content (where it has a
 *     testid), and
 *   - snapshots it (Layer B) — by content testid when the kit forwards one, else
 *     full-page (Popover/Tooltip/Select listbox don't expose a content testid).
 *
 * Backend-free via the gallery Vite server. Animations are disabled (config), so
 * the open state is deterministic.
 */
import { expect, test } from '@playwright/test'
import { assertLayoutSane } from '../helpers/layout'
import { SNAPSHOTS_ENABLED, openGallery } from './_gallery'

type OpenKind = 'click' | 'hover'

interface OverlayCase {
  name: string
  /** testid of the trigger to open the overlay. */
  trigger: string
  /** how to activate it. */
  kind?: OpenKind
  /** content testid the kit forwards to the portal root (enables a localized
   *  shot + layout assertion); null → snapshot full page. */
  content?: string
  /** a role to wait on when there's no content testid (e.g. select listbox). */
  waitRole?: string
}

const OVERLAYS: OverlayCase[] = [
  { name: 'dialog', trigger: 'g-dialog-open', content: 'g-dialog' },
  { name: 'sheet', trigger: 'g-sheet-open', content: 'g-sheet' },
  { name: 'confirm', trigger: 'g-confirm-open', content: 'g-confirm' },
  { name: 'dropdown', trigger: 'g-dropdown-open', content: 'g-dropdown' },
  // Select opens a Radix listbox (no content testid) — wait on role, shoot full page.
  { name: 'select', trigger: 'g-sel-filled', waitRole: 'listbox' },
  { name: 'combobox', trigger: 'g-cmb-default', waitRole: 'dialog' },
  { name: 'multiselect', trigger: 'g-ms-empty', waitRole: 'dialog' },
  { name: 'popover', trigger: 'g-popover-open', waitRole: 'dialog' },
]

for (const theme of ['light', 'dark'] as const) {
  test(`overlays open — ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 })
    await openGallery(page, theme, 'blue')

    for (const o of OVERLAYS) {
      await test.step(o.name, async () => {
        const trigger = page.getByTestId(o.trigger)
        await trigger.scrollIntoViewIfNeeded()
        if (o.kind === 'hover') await trigger.hover()
        else await trigger.click()

        // Resolve a handle to the open content: the kit's forwarded content
        // testid when available, else the portal's ARIA role (listbox/dialog).
        const content = o.content
          ? page.getByTestId(o.content)
          : o.waitRole
            ? page.getByRole(o.waitRole as 'dialog').first()
            : null
        // Wait for it to settle — if the trigger failed to open, this times out
        // and FAILS (no catch), so "opened" is genuinely asserted.
        if (content) await content.waitFor({ state: 'visible' })

        // Layer A — invariants on the open content for EVERY overlay (incl. the
        // role-resolved listbox/dialog cases). Overlays are dense layout surfaces;
        // this is where header/body/footer/action/option alignment bugs live.
        if (content) {
          await assertLayoutSane(content, { checks: { horizontalScroll: false } })
        }

        // Layer B — snapshot the open overlay (opt-in; needs blessed baselines).
        if (SNAPSHOTS_ENABLED) {
          const shot = content ?? page
          await expect(shot).toHaveScreenshot(`overlay-${o.name}-${theme}.png`)
        }

        // Close so the next overlay opens clean.
        await page.keyboard.press('Escape')
        if (content)
          await content.waitFor({ state: 'hidden' }).catch(() => undefined)
      })
    }
  })
}
