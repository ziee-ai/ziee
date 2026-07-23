/**
 * TEST-1..4 (ITEM-1, ITEM-2) — the sidebar user widget names the PERSON, not the
 * login handle.
 *
 * The widget used to render `user.username` in both of its render spots (the
 * SidebarItem label and the collapsed-sidebar Tooltip), so a user with a display
 * name still saw their username at the bottom of the sidebar. It now renders
 * `display_name` with a `||` fallback to `username`.
 *
 * Backend-free, against `/gallery.html` (playwright.visual.config boots vite).
 * The gallery seeds the auth store directly, so each case is explicit rather
 * than depending on the `?auth=` role semantics.
 */
import { test, expect, type Page } from '@playwright/test'

const DISPLAY_NAME = 'Ada Lovelace'
/**
 * The username these surfaces seed — deliberately NOT the cassette fixture's
 * `admin`, so an assertion on it can only pass if the seed actually applied.
 */
const USERNAME = 'alovelace'

/**
 * Open a seeded surface and return its frame plus any runtime errors, mirroring
 * `gallery-gap-seed.spec.ts`'s harness: a crash marker or a console/page error
 * means the surface is broken regardless of what the text says.
 */
async function openSurface(page: Page, slug: string) {
  const errors: string[] = []
  page.on('pageerror', e => errors.push(String(e)))
  page.on('console', m => {
    // Benign non-/api asset failures hit vite, not the mock — the runtime-health
    // gate filters these too; a real bug is a JS error.
    if (m.type() === 'error' && !/Failed to load resource/i.test(m.text()))
      errors.push(m.text())
  })
  await page.goto(`/gallery.html?surface=${slug}&theme=light&accent=blue`)
  await page.getByTestId('gallery-root').waitFor()
  const frame = page.getByTestId(`gallery-page-${slug}`)
  await frame.waitFor({ timeout: 15000 })
  await expect(frame.getByTestId('gallery-crash')).toHaveCount(0)
  return { frame, errors }
}

test('TEST-1: the widget renders the display name, not the username', async ({
  page,
}) => {
  const { frame, errors } = await openSurface(
    page,
    'seeded-s5-user-profile-display-name',
  )

  // Exact text, not a substring: the regression is that the row showed the
  // login handle, and `not.toContainText(USERNAME)` would also pass for a row
  // rendering both, while false-failing for any display name that happens to
  // contain the username.
  const row = frame.getByTestId('user-profile-widget')
  await expect(row).toHaveText(DISPLAY_NAME)
  expect(errors, `console/page errors: ${errors.join(' | ')}`).toEqual([])
})

test('TEST-2: a null display name falls back to the username', async ({
  page,
}) => {
  const { frame, errors } = await openSurface(
    page,
    'seeded-s5-user-profile-no-display-name',
  )

  // Exact text again: the fallback must produce the username and nothing else
  // (never a blank row). USERNAME is the seed's own, not the bootstrap
  // fixture's, so this cannot pass unless the seed applied.
  const row = frame.getByTestId('user-profile-widget')
  await expect(row).toHaveText(USERNAME)
  expect(errors, `console/page errors: ${errors.join(' | ')}`).toEqual([])
})

test('TEST-2b: a blank display name also falls back (|| not ??)', async ({
  page,
}) => {
  const { frame } = await openSurface(
    page,
    'seeded-s5-user-profile-blank-display-name',
  )

  // The case the `||`-over-`??` decision exists for: a whitespace-only
  // display_name (reachable via the admin create/update path, which stores
  // what it is given). With `??` the row would render blank.
  await expect(frame.getByTestId('user-profile-widget')).toHaveText(USERNAME)
})

test('TEST-3: the display name is the row accessible name, not just visible text', async ({
  page,
}) => {
  const { frame } = await openSurface(
    page,
    'seeded-s5-user-profile-display-name',
  )

  // SidebarItem feeds `label` into aria-label AND title as well as the visible
  // span, so a screen-reader user must hear the display name too. Asserting
  // only the text would let a half-fix (visible span only) pass. Base UI
  // tooltips are visual-only (no a11y-tree node), so this aria-label IS the
  // name in every state, collapsed included.
  await expect(
    frame.locator(`[aria-label="${DISPLAY_NAME}"]`),
  ).toHaveAccessibleName(DISPLAY_NAME)
  await expect(frame.locator(`[title="${DISPLAY_NAME}"]`)).toHaveCount(1)
})

test('TEST-4: the collapsed-sidebar tooltip carries the same label', async ({
  page,
}) => {
  const { frame, errors } = await openSurface(
    page,
    'seeded-s5-user-profile-collapsed',
  )

  const row = frame.getByTestId('user-profile-widget')
  const tooltip = page.locator('[data-slot="tooltip-content"]')

  // Settle the race BEFORE hovering: `setup` is fire-and-forget and runs in
  // parallel with the lazy component's mount, so the widget can first render
  // expanded (no Tooltip wrapper). `.hover()` is one-shot — hovering that tree
  // would never re-fire once the seed swaps in the wrapped one. This retrying
  // assertion is also the proof that the COLLAPSED branch is what rendered:
  // `data-tooltip-wrapped` is the kit Tooltip's marker, carrying its content.
  await expect(row).toHaveAttribute('data-tooltip-wrapped', DISPLAY_NAME)

  await row.hover()

  // The tooltip must actually OPEN, not merely be configured: it previously
  // wrapped <Dropdown>, which drops unknown props, so hovering rendered
  // nothing at all and the collapsed sidebar named the user nowhere on screen.
  await expect(tooltip).toBeVisible({ timeout: 10000 })
  await expect(tooltip).toContainText(DISPLAY_NAME)
  expect(errors, `console/page errors: ${errors.join(' | ')}`).toEqual([])
})
