import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, createTestUser, login, getCurrentUserToken, clearAuthState } from '../../common/auth-helpers'

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
      await expect(page.getByRole('heading', { name: 'Literature Search' })).toBeVisible({ timeout: 10000 })
      return
    } catch (e) {
      if (attempt === 3) throw e
      await page.waitForTimeout(1000)
    }
  }
}

function freshState(): State {
  return { settings: defaultSettings(), catalog: defaultCatalog(), lastSettingsPatch: null, lastConnectorPatch: null }
}

test.describe('Literature search admin settings', () => {
  test.describe.configure({ retries: 2 })

  test('loads general card + descriptor-driven connector cards', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await expect(page.getByText('Enable literature search')).toBeVisible()
    // `exact` — the connector's name renders in the divider header AND inside its
    // keyless_note paragraph, so a substring match resolves to 2 elements (matches
    // the CORE assertion just below, which already scopes with exact).
    await expect(page.getByText('Europe PMC', { exact: true })).toBeVisible()
    await expect(page.getByText('CORE', { exact: true })).toBeVisible()
    // CORE (required key, unset) shows the "Needs key" tag.
    await expect(page.getByText('Needs key')).toBeVisible()
    // The descriptor-driven config field's `help` text renders (proves the
    // generic catalog→UI contract covers help/docs_url, not just labels).
    await expect(page.getByText('Joins the Crossref polite pool for higher limits')).toBeVisible()
  })

  test('toggling the completeness switch persists', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await page.getByRole('switch', { name: 'Show completeness estimate' }).click()
    await expect(page.getByText('Completeness estimate updated')).toBeVisible({ timeout: 5000 })
    expect(state.lastSettingsPatch?.completeness_estimate_enabled).toBe(false)
  })

  test('setting the CORE key never echoes the secret + marks it configured', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    const SECRET = 'CORE-secret-e2e-xyz'
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await page.getByLabel('CORE API key').fill(SECRET)
    // The CORE connector's own Save button (scoped to the form with the key field).
    await page
      .locator('form')
      .filter({ has: page.getByLabel('CORE API key') })
      .getByRole('button', { name: 'Save' })
      .click()
    await expect(page.getByText('CORE saved')).toBeVisible({ timeout: 5000 })

    expect(state.lastConnectorPatch?.connector).toBe('core')
    expect(state.lastConnectorPatch?.body.api_key).toBe(SECRET)
    await expect(page.getByText(SECRET)).toHaveCount(0)
  })

  test('setting the Crossref mailto persists as config', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await page.getByLabel('Contact email').fill('researcher@example.org')
    await page
      .locator('form')
      .filter({ has: page.getByLabel('Contact email') })
      .getByRole('button', { name: 'Save' })
      .click()
    await expect(page.getByText('Crossref saved')).toBeVisible({ timeout: 5000 })
    expect(state.lastConnectorPatch?.connector).toBe('crossref')
    expect(state.lastConnectorPatch?.body.config.mailto).toBe('researcher@example.org')
  })

  test('stored mailto pre-fills and round-trips on save (no silent data loss)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState() // crossref has config.mailto = 'stored@example.org'
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    // The form must PRE-FILL the stored value (not start blank).
    await expect(page.getByLabel('Contact email')).toHaveValue('stored@example.org')

    // Saving WITHOUT retyping must round-trip it — not wipe it to '' (the bug).
    // Save is dirty-gated (`disabled={!canManage || !dirty}`), and re-`fill`ing the
    // SAME value doesn't fire antd's `onValuesChange` → Save stays disabled and the
    // click times out. Clear then restore the stored value to force `dirty`, while
    // still verifying the save round-trips the stored value (not '').
    await page.getByLabel('Contact email').fill('')
    await page.getByLabel('Contact email').fill('stored@example.org')
    await page
      .locator('form')
      .filter({ has: page.getByLabel('Contact email') })
      .getByRole('button', { name: 'Save' })
      .click()
    await expect(page.getByText('Crossref saved')).toBeVisible({ timeout: 5000 })
    expect(state.lastConnectorPatch?.body.config.mailto).toBe('stored@example.org')
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
    await expect(page.getByText(/Not authorized/i)).toBeVisible({ timeout: 8000 })
    expect(page.url()).toContain('/settings/literature')
    await expect(page.getByRole('heading', { name: 'Literature Search' })).toHaveCount(0)
  })

  test('master enable toggle persists', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    const masterSwitch = page.getByRole('switch', { name: 'Enable literature search' })
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

    await page.getByRole('switch', { name: 'Enable CORE' }).click()
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

    await page.getByRole('button', { name: 'Clear key' }).click()
    await expect.poll(() => state.lastConnectorPatch?.connector).toBe('core')
    expect(state.lastConnectorPatch?.body.api_key).toBe('')
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
    await expect(
      page.getByText('Failed to load literature search settings'),
    ).toBeVisible({ timeout: 10000 })
  })

  test('caps form saves max_results / per-source / timeout', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state = freshState()
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoLiterature(page, baseURL)

    await page.getByLabel('Max deduped results').fill('42')
    await page.getByRole('button', { name: 'Save caps' }).click()
    await expect(page.getByText('Literature search settings saved')).toBeVisible({ timeout: 5000 })
    expect(state.lastSettingsPatch?.max_results).toBe(42)
  })
})
