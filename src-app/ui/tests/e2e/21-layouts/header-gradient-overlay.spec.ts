import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — HeaderBarContainer soft-fade gradient overlay
 * (`HeaderBarContainer.tsx:46-57`): an aria-hidden div positioned at the
 * header's bottom edge with a `linear-gradient(...)` background. Untested. We
 * assert the overlay element is present and actually carries a linear-gradient
 * background image.
 */

test.describe('App layout — header fade gradient overlay', () => {
  test('the header renders an aria-hidden linear-gradient fade overlay', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    // The app shell (with the header bar) is mounted; the fade overlay is an
    // aria-hidden div carrying a linear-gradient background positioned at the
    // header's bottom edge (top: 100%).
    const overlay = page.getByTestId('layout-header-fade-overlay').first()
    await expect(overlay).toBeAttached({ timeout: 30000 })
    await expect
      .poll(
        async () =>
          overlay.evaluate(n => {
            const s = getComputedStyle(n as HTMLElement)
            return (
              s.backgroundImage.includes('linear-gradient') &&
              (n as HTMLElement).style.top === '100%'
            )
          }),
        { timeout: 30000 },
      )
      .toBe(true)
  })
})
