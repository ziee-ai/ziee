/**
 * E2E — editing an EXISTING auth provider and SAVING a config change.
 *
 * Audit gap (all-98271c684f16): `auth-providers-crud.spec.ts` opens the
 * Edit drawer but closes it without saving (it only asserts the Name
 * slug is disabled, then Cancel). Nothing exercised the actual
 * edit→Save→persist path through AuthProviderEditDrawer's editable
 * config fields (`config.client_id` etc.).
 *
 * This creates a Generic OIDC provider, reopens its Edit drawer, changes
 * the Client ID to a beacon value, clicks Save, asserts the real
 * `PUT /api/admin/auth-providers/{id}` fires, and then reopens the drawer
 * to confirm the new value persisted. No API mocks — real backend.
 */
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

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
    await byTestId(page, 'authprov-add-button').click()
    await byTestId(page, 'authprov-add-dropdown-item-oidc-generic').click()
    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible({
      timeout: 10_000,
    })
    await byTestId(page, 'authprov-name-input').fill(providerName)
    await byTestId(page, 'authprov-oidc-client-id-input').fill('original-client-id')
    await byTestId(page, 'authprov-oidc-client-secret-input').fill('e2e-secret-value')
    await byTestId(page, 'authprov-oidc-issuer-url-input').fill(
      'https://nonexistent.invalid/oidc',
    )
    await byTestId(page, 'authprov-drawer-save-button').click()

    await expect(
      byTestId(page, `authprov-toggle-switch-${providerName}`),
    ).toBeVisible({ timeout: 10_000 })

    // -------------------- EDIT + SAVE a config field --------------------
    await byTestId(page, `authprov-edit-button-${providerName}`).click()
    const clientIdInput = byTestId(page, 'authprov-oidc-client-id-input')
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
    await byTestId(page, 'authprov-drawer-save-button').click()
    const resp = await savePut
    expect(resp.ok(), `PUT status ${resp.status()}`).toBeTruthy()

    // -------------------- REOPEN → value persisted --------------------
    await byTestId(page, `authprov-edit-button-${providerName}`).click()
    await expect(byTestId(page, 'authprov-oidc-client-id-input')).toHaveValue(
      beaconClientId,
      { timeout: 10_000 },
    )
    await byTestId(page, 'authprov-drawer-cancel-button').click()

    // -------------------- CLEANUP --------------------
    await byTestId(page, `authprov-delete-button-${providerName}`).click()
    await byTestId(
      page,
      `authprov-delete-confirm-${providerName}-confirm`,
    ).click()
    await expect(
      byTestId(page, `authprov-toggle-switch-${providerName}`),
    ).toHaveCount(0, { timeout: 30_000 })
  })
})
