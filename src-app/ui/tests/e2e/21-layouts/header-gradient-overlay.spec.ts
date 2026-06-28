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

    // The app shell (with the header bar) is mounted; find an aria-hidden
    // overlay whose computed background image is a linear gradient.
    await expect
      .poll(
        async () =>
          page.evaluate(() => {
            const nodes = Array.from(
              document.querySelectorAll('div[aria-hidden="true"]'),
            )
            return nodes.some(n => {
              const s = getComputedStyle(n as HTMLElement)
              return (
                s.backgroundImage.includes('linear-gradient') &&
                (n as HTMLElement).style.top === '100%'
              )
            })
          }),
        { timeout: 30000 },
      )
      .toBe(true)
  })
})
