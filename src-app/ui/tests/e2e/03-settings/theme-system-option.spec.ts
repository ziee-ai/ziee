import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { isDarkMode, getTheme } from '../../utils/theme'

/**
 * Selecting the "System" theme THROUGH THE UI (the Theme Select on
 * /settings/general) makes the app track the OS color-scheme. The existing
 * theme specs drive light/dark explicitly; the System option is never chosen
 * via the dropdown. Here we pick it from the Select, then flip the emulated OS
 * scheme and assert the document follows along (dark when the OS is dark, light
 * when light) — proving the System branch is wired, not just persisted.
 */
test.describe('Settings — System theme option', () => {
  test('choosing System from the Theme dropdown tracks the OS scheme', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/general`)
    await expect(page.getByRole('heading', { name: 'General' })).toBeVisible({
      timeout: 15000,
    })

    // Open the antd Theme Select and pick "System".
    await page.getByLabel('Theme').click()
    await page.getByRole('option', { name: 'System' }).click()

    // The persisted preference is now `system`.
    await expect.poll(() => getTheme(page)).toBe('system')

    // OS = dark → the document goes dark.
    await page.emulateMedia({ colorScheme: 'dark' })
    await expect.poll(() => isDarkMode(page)).toBe(true)

    // OS = light → the document goes light. (Same `system` preference, the
    // app reacts to the OS change — the behavior the System option exists for.)
    await page.emulateMedia({ colorScheme: 'light' })
    await expect.poll(() => isDarkMode(page)).toBe(false)
  })
})
