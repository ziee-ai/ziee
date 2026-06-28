import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — a key-REQUIRED connector (CORE) cannot be saved without a key.
 *
 * Audit gap (all-a6b7ebf6c45c): LitSearchConnectorsSection gates the per-
 * connector Save button with
 *   `disabled={!canManage || !dirty || (needsKey && !apiKeyValue?.trim())}`
 * and the api_key Form.Item carries a `{ required: true }` rule when the
 * connector's key is required-but-unset (`needsKey`). The existing
 * admin-settings.spec covers the happy path (setting the key saves) and the
 * "Needs key" tag, but NEVER asserts that the validation BLOCKS save when the
 * key is absent. This isolates that branch.
 *
 * Only the lit-search settings/connectors HTTP boundary is mocked; the
 * disabled-Save validation logic under test is the real component code.
 */

type Connector = {
  key: string
  display_name: string
  keyless_note: string
  key_field: { required: boolean; label: string; help?: string; docs_url?: string } | null
  config_fields: Array<{ key: string; label: string; required: boolean; placeholder: string }>
  enabled: boolean
  configured: boolean
  api_key_set: boolean
  config: Record<string, unknown>
}

function catalog(): Connector[] {
  return [
    {
      key: 'core',
      display_name: 'CORE',
      keyless_note: 'CORE requires a free API key.',
      key_field: {
        required: true,
        label: 'CORE API key',
        help: 'Register at core.ac.uk',
        docs_url: 'https://core.ac.uk',
      },
      config_fields: [],
      enabled: false,
      configured: false,
      api_key_set: false, // required key NOT set → needsKey === true
      config: {},
    },
  ]
}

async function mockApi(page: Page) {
  const settings = {
    enabled: true,
    enabled_connectors: ['core'],
    max_results: 25,
    per_source_limit: 50,
    request_timeout_secs: 30,
    completeness_estimate_enabled: true,
    updated_at: new Date().toISOString(),
  }
  await page.route(/\/api\/lit-search\/settings$/, route =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(settings) }),
  )
  await page.route(/\/api\/lit-search\/connectors$/, route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ connectors: catalog() }),
    }),
  )
}

test.describe('Literature — connector "Needs key" validation prevents save', () => {
  test.describe.configure({ retries: 2 })

  test('CORE Save stays disabled until a key is entered', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockApi(page)
    await page.goto(`${baseURL}/settings/literature`)
    await expect(
      page.getByRole('heading', { name: 'Literature Search' }),
    ).toBeVisible({ timeout: 10000 })

    // CORE (required key, unset) advertises that it needs a key.
    await expect(page.getByText('Needs key')).toBeVisible()

    const coreForm = page
      .locator('form')
      .filter({ has: page.getByLabel('CORE API key') })
    const saveBtn = coreForm.getByRole('button', { name: 'Save' })

    // No key entered → Save is disabled (cannot persist an invalid CORE config).
    await expect(saveBtn).toBeDisabled()

    // Dirtying the form by typing then clearing the key MUST still leave Save
    // disabled — isolating the `needsKey && !apiKeyValue` branch from the
    // separate dirty-gate.
    const keyField = page.getByLabel('CORE API key')
    await keyField.fill('temp')
    await expect(saveBtn).toBeEnabled()
    await keyField.fill('')
    await expect(saveBtn).toBeDisabled()

    // Entering a real key re-enables Save (the validation clears).
    await keyField.fill('CORE-key-123')
    await expect(saveBtn).toBeEnabled()
  })
})
