import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-38c5b66f1ef8 — the HeaderBarContainer soft-fade overlay
// (HeaderBarContainer.tsx:46-57) is an aria-hidden div rendered just below the
// header with a `linear-gradient(to bottom, <bg>, <fadeOut>)` background. It had
// no test, so a regression that drops the overlay (or its gradient) would be
// silent. This asserts the overlay is present in the app shell and that its
// COMPUTED background is actually a linear gradient (not a flat color / empty).
test.describe('Header bar fade overlay', () => {
  test('renders an aria-hidden linear-gradient fade overlay below the header', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Any in-app route mounts the app layout + HeaderBarContainer.
    await page.goto(`${baseURL}/settings/general`)
    await expect(page.getByText('Appearance')).toBeVisible({ timeout: 30000 })

    // The overlay is the inline-styled fade div. Match it by its distinctive
    // gradient declaration so we don't depend on classnames.
    const overlay = page
      .locator('div[aria-hidden="true"][style*="linear-gradient(to bottom"]')
      .first()
    await expect(overlay).toBeAttached({ timeout: 10000 })

    // The COMPUTED background image must be a real gradient (proves the token
    // colors resolved, not an empty/flat background).
    const bgImage = await overlay.evaluate(
      (el) => getComputedStyle(el).backgroundImage,
    )
    expect(bgImage).toContain('linear-gradient')

    // It is decorative: hidden from the accessibility tree and non-interactive.
    await expect(overlay).toHaveAttribute('aria-hidden', 'true')
    const pointerEvents = await overlay.evaluate(
      (el) => getComputedStyle(el).pointerEvents,
    )
    expect(pointerEvents).toBe('none')
  })
})
