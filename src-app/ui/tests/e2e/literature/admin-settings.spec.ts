import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, createTestUser, login, getCurrentUserToken, clearAuthState } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// lit_search admin config lives on a dedicated page (the built-in MCP row is
// hidden from the System MCP page). Mock the settings singleton + the
// descriptor-driven connector catalog, tracking state so a reload reflects saves.

type Settings = {
  enabled: boolean
  enabled_connectors: string[]
  max_results: number
  per_source_limit: number
  request_timeout_secs: number
  completeness_estimate_enabled: boolean
  updated_at: string
}

type KeyField = { required: boolean; label: string; help?: string; docs_url?: string }
type ConfigField = {
  key: string
  label: string
  required: boolean
  placeholder: string
  help?: string
  docs_url?: string
}
type Connector = {
  key: string
  display_name: string
  keyless_note: string
  key_field: KeyField | null
  config_fields: ConfigField[]
  enabled: boolean
  configured: boolean
  api_key_set: boolean
  config: Record<string, unknown>
}

function defaultSettings(): Settings {
  return {
    enabled: true,
    enabled_connectors: ['europepmc', 'crossref'],
    max_results: 25,
    per_source_limit: 50,
    request_timeout_secs: 30,
    completeness_estimate_enabled: true,
    updated_at: new Date().toISOString(),
  }
}

function defaultCatalog(): Connector[] {
  return [
    {
      key: 'europepmc',
      display_name: 'Europe PMC',
      keyless_note: 'Works without a key.',
      key_field: null,
      config_fields: [],
      enabled: true,
      configured: true,
      api_key_set: false,
      config: {},
    },
    {
      key: 'crossref',
      display_name: 'Crossref',
      keyless_note: 'Add a contact email to join the polite pool.',
      key_field: null,
      config_fields: [
        {
          key: 'mailto',
          label: 'Contact email',
          required: false,
          placeholder: 'you@example.org',
          help: 'Joins the Crossref polite pool for higher limits',
        },
      ],
      enabled: true,
      configured: true,
      api_key_set: false,
      // A stored mailto — the form must pre-fill + round-trip this, not wipe it.
      config: { mailto: 'stored@example.org' },
    },
    {
      key: 'core',
      display_name: 'CORE',
      keyless_note: 'CORE requires a free API key.',
      key_field: { required: true, label: 'CORE API key', help: 'Register at core.ac.uk', docs_url: 'https://core.ac.uk' },
      config_fields: [],
      enabled: false,
      configured: false,
      api_key_set: false,
      config: {},
    },
  ]
}

type State = {
  settings: Settings
  catalog: Connector[]
  lastSettingsPatch: Partial<Settings> | null
  lastConnectorPatch: { connector: string; body: any } | null
  // The Sources card now saves ALL connectors in ONE submit (commit 935af6ac:
  // "one Save for all Sources"), so a single save fires a PATCH per connector.
  // Keep a per-connector map so a test can assert the payload of the specific
  // connector it edited, independent of iteration order.
  connectorPatches: Record<string, any>
}

async function mockApi(page: Page, state: State) {
  await page.route(/\/api\/lit-search\/settings$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(state.settings) })
    }
    if (req.method() === 'PUT') {
      const patch = JSON.parse(req.postData() ?? '{}') as Partial<Settings>
      state.lastSettingsPatch = patch
      state.settings = { ...state.settings, ...patch, updated_at: new Date().toISOString() }
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(state.settings) })
    }
    return route.continue()
  })

  await page.route(/\/api\/lit-search\/connectors$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ connectors: state.catalog }) })
    }
    return route.continue()
  })

  await page.route(/\/api\/lit-search\/connectors\/[^/]+$/, async (route, req) => {
    if (req.method() === 'PUT') {
      const connector = req.url().split('/').pop() as string
      const body = JSON.parse(req.postData() ?? '{}')
      state.lastConnectorPatch = { connector, body }
      state.connectorPatches[connector] = body
      state.catalog = state.catalog.map(c => {
        if (c.key !== connector) return c
        const hasKey = body.api_key !== undefined ? body.api_key.length > 0 : c.api_key_set
        return { ...c, api_key_set: hasKey, configured: c.key_field?.required ? hasKey : true }
      })
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ connectors: state.catalog }) })
    }
    return route.continue()
  })
}

async function gotoLiterature(page: Page, baseURL: string) {
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      await page.goto(`${baseURL}/settings/literature`)
      // The page is a two-tab layout (General | Sources). The tabs shell always
      // renders once the page mounts, so it's the stable "page mounted" signal
      // (the General tab is selected by default; the Sources card lives behind
      // its own tab, which shadcn Tabs unmounts until selected).
      await expect(byTestId(page, 'lit-settings-tabs')).toBeVisible({ timeout: 10000 })
      return
    } catch (e) {
      if (attempt === 3) throw e
      await page.waitForTimeout(1000)
    }
  }
}

// The connector cards live under the "Sources" tab, which shadcn Tabs keeps
// unmounted until selected. Switch to it (and wait for the card) before
// touching any lit-connector-* control.
async function openSourcesTab(page: Page) {
  await page.getByRole('tab', { name: 'Sources' }).click()
  await expect(byTestId(page, 'lit-connectors-card')).toBeVisible({ timeout: 10000 })
}

function freshState(): State {
  return {
    settings: defaultSettings(),
    catalog: defaultCatalog(),
    lastSettingsPatch: null,
    lastConnectorPatch: null,
    connectorPatches: {},
  }
}

test.describe('Literature search admin settings', () => {
  test.describe.configure({ retries: 2 })

  test('loads general card + descriptor-driven connector cards', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    // General tab (default) carries the master enable switch.
    await expect(byTestId(page, 'lit-global-enable-switch')).toBeVisible()

    // The descriptor-driven connector cards render under the Sources tab
    // (Europe PMC + CORE).
    await openSourcesTab(page)
    await expect(byTestId(page, 'lit-connector-enable-switch-europepmc')).toBeVisible()
    await expect(byTestId(page, 'lit-connector-enable-switch-core')).toBeVisible()
    // CORE (required key, unset) shows the "Needs key" tag.
    await expect(byTestId(page, 'lit-connector-needs-key-tag-core')).toBeVisible()
    // The descriptor-driven config field's `help` text renders (proves the
    // generic catalog→UI contract covers help/docs_url, not just labels). The
    // kit FormField derives its description testid from the field `name`, which
    // is namespaced `<connector>.<field>` → `field-desc-crossref.mailto`.
    await expect(byTestId(page, 'field-desc-crossref.mailto')).toContainText(
      'Joins the Crossref polite pool',
    )
  })

  test('toggling the completeness switch persists', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await byTestId(page, 'lit-global-completeness-switch').click()
    await expect
      .poll(() => state.lastSettingsPatch?.completeness_estimate_enabled, { timeout: 5000 })
      .toBe(false)
  })

  test('setting the CORE key never echoes the secret + marks it configured', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    const SECRET = 'CORE-secret-e2e-xyz'
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await openSourcesTab(page)
    await byTestId(page, 'lit-connector-api-key-input-core').fill(SECRET)
    // One Save for the whole Sources card.
    await byTestId(page, 'lit-connectors-save').click()

    await expect.poll(() => state.connectorPatches['core']?.api_key, { timeout: 5000 }).toBe(SECRET)
    // The secret is never echoed back into the page (the key field resets blank).
    await expect(byTestId(page, 'lit-connector-api-key-input-core')).toHaveValue('')
    await expect(page.locator('body')).not.toContainText(SECRET)
  })

  test('setting the Crossref mailto persists as config', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await openSourcesTab(page)
    await byTestId(page, 'lit-connector-config-input-crossref-mailto').fill('researcher@example.org')
    await byTestId(page, 'lit-connectors-save').click()
    await expect
      .poll(() => state.connectorPatches['crossref']?.config?.mailto, { timeout: 5000 })
      .toBe('researcher@example.org')
  })

  test('stored mailto pre-fills and round-trips on save (no silent data loss)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState() // crossref has config.mailto = 'stored@example.org'
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await openSourcesTab(page)
    // The form must PRE-FILL the stored value (not start blank).
    const mailto = byTestId(page, 'lit-connector-config-input-crossref-mailto')
    await expect(mailto).toHaveValue('stored@example.org')

    // The mailto field must be READ into the save payload (not silently dropped
    // to '' — the historical data-loss bug). Save is dirty-gated
    // (`disabled={!canManage || !form.formState.isDirty}`); react-hook-form's
    // isDirty compares against defaultValues, so clearing then restoring the
    // SAME value leaves the form pristine (Save disabled). Change it to a new
    // value to make the form dirty, then assert THAT value round-trips into
    // `config.mailto` — proving the field flows to the save payload.
    const roundtripped = 'roundtrip@example.org'
    await mailto.fill(roundtripped)
    await byTestId(page, 'lit-connectors-save').click()
    await expect
      .poll(() => state.connectorPatches['crossref']?.config?.mailto, { timeout: 5000 })
      .toBe(roundtripped)
  })

  test('a non-admin cannot reach the literature settings page', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getCurrentUserToken(page)
    await createTestUser(apiURL, adminToken, 'lit_plain', 'lit_plain@example.com', 'Password123!', [])
    await clearAuthState(page)
    await login(page, baseURL, 'lit_plain', 'Password123!')

    await page.goto(`${baseURL}/settings/literature`)
    // Inline 403 with the URL preserved — the POSITIVE proof the LitSearchAdminRead
    // route guard fired (not just a failed navigation). Mirrors the permissions/ E2E.
    await expect(
      page.locator(
        '[data-testid="router-route-forbidden-result"], [data-testid="settings-forbidden-result"]',
      ),
    ).toBeVisible({ timeout: 8000 })
    expect(page.url()).toContain('/settings/literature')
    await expect(byTestId(page, 'lit-global-card')).toHaveCount(0)
  })

  test('master enable toggle persists', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    const masterSwitch = byTestId(page, 'lit-global-enable-switch')
    await expect(masterSwitch).toBeChecked()
    await masterSwitch.click()
    await expect.poll(() => state.lastSettingsPatch?.enabled).toBe(false)
    await expect(masterSwitch).not.toBeChecked()
  })

  test('per-source enable toggle adds the connector to enabled_connectors', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState() // CORE starts disabled
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await openSourcesTab(page)
    // Enable is a staged form field now; the Save commits enabled_connectors via
    // the settings PUT (onSaveAll).
    await byTestId(page, 'lit-connector-enable-switch-core').click()
    await byTestId(page, 'lit-connectors-save').click()
    await expect.poll(() => state.lastSettingsPatch?.enabled_connectors).toContain('core')
    // The originally-enabled sources are preserved (not clobbered).
    expect(state.lastSettingsPatch?.enabled_connectors).toEqual(
      expect.arrayContaining(['europepmc', 'crossref', 'core']),
    )
  })

  test('clear-key flow removes a stored CORE key', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    // Pre-set the CORE key so the "Clear key" button renders.
    state.catalog = state.catalog.map(c =>
      c.key === 'core' ? { ...c, api_key_set: true, configured: true } : c,
    )
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await openSourcesTab(page)
    // Clear-key fires its own immediate PATCH (not routed through the card Save).
    await byTestId(page, 'lit-connector-clear-key-button-core').click()
    await expect.poll(() => state.connectorPatches['core']?.api_key).toBe('')
  })

  test('shows an error Alert when settings fail to load', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // Settings endpoint hard-fails → the store surfaces `error`, which the
    // page renders as an error Alert above the cards.
    await page.route(/\/api\/lit-search\/settings$/, async route =>
      route.fulfill({ status: 500, contentType: 'application/json', body: JSON.stringify({ message: 'boom' }) }),
    )
    await page.route(/\/api\/lit-search\/connectors$/, async route =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ connectors: [] }) }),
    )

    await page.goto(`${baseURL}/settings/literature`)
    await expect(byTestId(page, 'lit-settings-error-alert')).toBeVisible({ timeout: 10000 })
  })

  test('caps form saves max_results / per-source / timeout', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await byTestId(page, 'lit-global-max-results-input').fill('42')
    await byTestId(page, 'lit-global-save-caps-button').click()
    await expect.poll(() => state.lastSettingsPatch?.max_results, { timeout: 5000 }).toBe(42)
  })

  // audit id 200f6ab3e2c9 — a connector whose key_field is required and has no
  // stored key (CORE) carries a required-rule on its api_key field; saving with
  // it empty must be blocked by inline validation, NOT sent to the server.
  test("a required-key connector blocks save when the key is empty", async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState() // CORE: key_field.required=true, api_key_set=false
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await openSourcesTab(page)
    // CORE shows the "Needs key" tag (the warning state).
    await expect(byTestId(page, 'lit-connector-needs-key-tag-core')).toBeVisible()

    // The Sources card now has one Save for all connectors (commit 935af6ac).
    // Saving with CORE's key left empty must NEVER transmit an empty/blank key:
    // onSaveAll omits api_key entirely for a required connector whose field is
    // blank (`if (c.key_field && apiKey)`), so an invalid CORE key is never
    // persisted. Assert the invariant on the payload rather than a removed
    // per-connector disabled-Save gate.
    await byTestId(page, 'lit-connectors-save').click()
    // Wait for the save-all to have fired (the settings PUT always goes).
    await expect.poll(() => state.lastSettingsPatch, { timeout: 5000 }).not.toBeNull()
    // CORE's PATCH (if any) carries no api_key — the blank required key is dropped.
    expect(state.connectorPatches['core']?.api_key).toBeUndefined()
    // CORE stays flagged as needing a key (still unconfigured).
    await expect(byTestId(page, 'lit-connector-needs-key-tag-core')).toBeVisible()
  })

  // audit id bfae0a63e1633179 — the page's load-error branch
  // (LitSearchSettingsPage.tsx:20-28, the error Alert) had no E2E scenario.
  test('shows the error Alert when settings fail to load', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Settings GET fails → the store sets `error` → the page renders its Alert.
    await page.route(/\/api\/lit-search\/settings$/, async (route, req) => {
      if (req.method() === 'GET') {
        return route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error_code: 'INTERNAL', error: 'settings load exploded' }),
        })
      }
      return route.continue()
    })
    // Keep the connector catalog GET well-formed so only the settings path errors.
    await page.route(/\/api\/lit-search\/connectors$/, async (route, req) => {
      if (req.method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ connectors: defaultCatalog() }),
        })
      }
      return route.continue()
    })

    await gotoLiterature(page, baseURL)
    await expect(byTestId(page, 'lit-settings-error-alert')).toBeVisible({ timeout: 10000 })
  })
})
