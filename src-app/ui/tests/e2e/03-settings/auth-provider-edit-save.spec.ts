/**
 * E2E — editing an EXISTING auth provider and SAVING a config change.
 *
 * Audit gap (all-98271c684f16): `auth-providers-crud.spec.ts` opens the
 * Edit drawer but closes it without saving (it only asserts the Name
 * slug is disabled, then Cancel). Nothing exercised the actual
 * edit→Save→persist path through AuthProviderEditDrawer's editable
 * config fields (`config.client_id` etc., Save button label "Save").
 *
 * This creates a Generic OIDC provider, reopens its Edit drawer, changes
 * the Client ID to a beacon value, clicks Save, asserts the real
 * `PUT /api/admin/auth-providers/{id}` fires, and then reopens the drawer
 * to confirm the new value persisted. No API mocks — real backend.
 */
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

const ADD_PROVIDER = 'Add authentication provider'

test.describe('Auth providers — edit existing provider saves config', () => {
  test('changing Client ID via the Edit drawer persists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-edit-${Date.now()}`
    const beaconClientId = `edited-client-${Date.now().toString(36)}`

    // -------------------- CREATE (disabled; fake issuer) --------------------
    await page.getByRole('button', { name: ADD_PROVIDER }).click()
    await page.getByRole('menuitem', { name: /Generic OIDC/i }).click()
    await expect(page.getByRole('button', { name: /^Create$/ })).toBeVisible({
      timeout: 10_000,
    })
    await page.getByLabel(/Name \(URL slug\)/i).fill(providerName)
    await page.getByLabel(/Client ID/i).fill('original-client-id')
    await page.locator('input[type="password"]').first().fill('e2e-secret-value')
    await page.getByLabel(/Issuer URL/i).fill('https://nonexistent.invalid/oidc')
    await page.getByRole('button', { name: /^Create$/ }).click()

    await expect(
      page.getByRole('switch', { name: `Toggle ${providerName}` }),
    ).toBeVisible({ timeout: 10_000 })

    // -------------------- EDIT + SAVE a config field --------------------
    await page.getByRole('button', { name: `Edit ${providerName}` }).click()
    const clientIdInput = page.getByLabel(/Client ID/i)
    await expect(clientIdInput).toHaveValue('original-client-id', {
      timeout: 5_000,
    })
    await clientIdInput.fill(beaconClientId)

    const savePut = page.waitForResponse(
      r =>
        /\/api\/admin\/auth-providers\/[^/]+$/.test(r.url()) &&
        r.request().method() === 'PUT',
      { timeout: 15_000 },
    )
    await page.getByRole('button', { name: /^Save$/ }).click()
    const resp = await savePut
    expect(resp.ok(), `PUT status ${resp.status()}`).toBeTruthy()

    // -------------------- REOPEN → value persisted --------------------
    await page.getByRole('button', { name: `Edit ${providerName}` }).click()
    await expect(page.getByLabel(/Client ID/i)).toHaveValue(beaconClientId, {
      timeout: 10_000,
    })
    await page.getByRole('button', { name: /^Cancel$/ }).click()

    // -------------------- CLEANUP --------------------
    await page.getByRole('button', { name: `Delete ${providerName}` }).click()
    const popover = page.locator('.ant-popover:visible').last()
    await expect(popover).toBeVisible({ timeout: 5_000 })
    await popover.locator('.ant-btn-primary').click()
    await expect(
      page.getByRole('switch', { name: `Toggle ${providerName}` }),
    ).toHaveCount(0, { timeout: 30_000 })
  })
})
