/**
 * Permission-gating E2E for /settings/assistant-templates.
 *
 * The page + sidebar slot are gated on `assistant_templates::read`
 * (module.tsx route `permission` + the 'Assistant Templates' settingsAdminPages
 * slot). Every other assistant E2E logs in as the root admin; the NEGATIVE gate
 * (a non-admin without the read perm) was never asserted. no-403 fixture catches
 * any stray /api 403 during the member flow.
 */
import { test, expect } from './no-403'
import { loginAsMember, loginWithPerms } from './fixtures'
import { Permissions } from '../../../src/api-client/types'

test.describe('assistant-templates — permission gating', () => {
  test('member without assistant_templates::read: entry hidden + deep-link 403', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // The admin sidebar entry is gated → hidden.
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(
      page.getByRole('menuitem', { name: /^Assistant Templates$/ }),
    ).toHaveCount(0)

    // Deep-link → inline "Not authorized", URL preserved (route gate fires).
    await page.goto(`${testInfra.baseURL}/settings/assistant-templates`)
    await expect(page.getByText(/Not authorized/i)).toBeVisible({ timeout: 10_000 })
    expect(page.url()).toContain('/settings/assistant-templates')
  })

  test('reader with assistant_templates::read can open the templates page', async ({
    page,
    testInfra,
  }) => {
    await loginWithPerms(page, testInfra.baseURL, testInfra.apiURL, [
      Permissions.AssistantsTemplateRead,
    ])

    await page.goto(`${testInfra.baseURL}/settings/assistant-templates`)
    await expect(page.getByText(/Not authorized/i)).toHaveCount(0)
    // The templates page renders (its unique subtitle), proving access is granted.
    await expect(
      page.getByText(/Manage template assistants/i),
    ).toBeVisible({ timeout: 10_000 })
  })
})
