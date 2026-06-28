import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * HeaderBarContainer renders a soft-fade overlay just below the header — an
 * aria-hidden, pointer-events-none absolutely-positioned strip with a
 * linear-gradient background (content color → transparent). It had no test at
 * all. Rather than a brittle screenshot, this asserts the overlay element is
 * present in the app shell with the expected computed style (linear-gradient
 * background, position absolute, the 16px height, top:100%).
 */
test.describe('App shell — header fade overlay', () => {
  test('renders the linear-gradient fade overlay below the header', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // Any authenticated page carries the app-shell header.
    await page.goto(`${baseURL}/settings/general`)
    await expect(page.getByRole('heading', { name: 'General' })).toBeVisible({
      timeout: 15000,
    })

    // Find an aria-hidden overlay whose computed style matches the fade strip.
    const overlay = await page.evaluate(() => {
      const divs = Array.from(
        document.querySelectorAll<HTMLElement>('div[aria-hidden="true"]'),
      )
      const match = divs.find((d) => {
        const s = getComputedStyle(d)
        return (
          s.backgroundImage.includes('linear-gradient') &&
          s.position === 'absolute' &&
          s.height === '16px' &&
          s.pointerEvents === 'none'
        )
      })
      return match
        ? { backgroundImage: getComputedStyle(match).backgroundImage }
        : null
    })

    expect(overlay, 'header fade overlay element must exist').not.toBeNull()
    expect(overlay!.backgroundImage).toContain('linear-gradient')
  })
})
