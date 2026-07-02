import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import { setTheme, isDarkMode } from '../../utils/theme'

/**
 * E2E — dark-mode coverage for the standalone hardware MONITOR popup
 * (`/hardware-monitor`, BlankLayout). The existing hardware.spec only exercises
 * dark mode on the settings page; the popup route was untested in dark mode.
 */

test.describe('Hardware monitor popup — dark mode', () => {
  test('renders in dark mode and passes accessibility checks', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await setTheme(page, 'dark')
    await page.goto(`${baseURL}/hardware-monitor`)

    // The monitor renders its real-time status. Key off the CPU-usage card
    // (always present when hardware is available) rather than the connection
    // card — the latter is `className={sseConnected ? 'hidden' : 'block'}`, so
    // it DISAPPEARS once the real hardware SSE connects (which it does on a
    // real host), which would make this a flaky, connection-state-dependent gate.
    await expect(byTestId(page, 'hardware-cpu-card')).toBeVisible({
      timeout: 30000,
    })

    // Dark mode is actually active on this route.
    expect(await isDarkMode(page)).toBe(true)

    await assertNoAccessibilityViolations(page)
  })
})
