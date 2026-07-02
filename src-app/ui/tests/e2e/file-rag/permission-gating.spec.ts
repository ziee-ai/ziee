import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Document-RAG admin read-only permission gating (EnableSection.tsx
 * `disabled={!canManage}`).
 *
 * Audit gap: a user with `file_rag::admin::read` but NOT `::manage` can VIEW
 * the admin settings but must not be able to mutate them — the section
 * form is rendered disabled. This was untested. A read-only user visits the
 * page and the master enable Switch + Save are disabled.
 */

test.describe('Document RAG — read-only permission gating', () => {
  test('read-only admin sees the section but cannot manage it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const username = `fragRO_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit', 'file_rag::admin::read'],
    )
    await login(page, baseURL, username, 'password123')

    await page.goto(`${baseURL}/settings/file-rag-admin`)

    // The card renders (read is permitted)...
    const enableSwitch = byTestId(page, 'filerag-enable-switch')
    await expect(enableSwitch).toBeVisible({ timeout: 30000 })

    // ...but management controls are disabled (form `disabled={!canManage}`),
    // and the save/cancel actions are hidden entirely for non-managers — the
    // SettingsFormActions footer renders only when `canManage` (footer={canManage
    // ? <actions/> : undefined}), consistent across every settings card.
    await expect(enableSwitch).toBeDisabled()
    await expect(byTestId(page, 'filerag-enable-save')).toHaveCount(0)
  })
})
