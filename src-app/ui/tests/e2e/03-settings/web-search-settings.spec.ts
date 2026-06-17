import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// ---------------------------------------------------------------------------
// Route mocks. web_search admin settings = a singleton settings row + a
// per-provider catalog. We mock the GET/PUT for both, tracking state so a
// post-save reload reflects the change (mirrors sandbox-resource-limits.spec).
// ---------------------------------------------------------------------------

type Settings = {
  enabled: boolean
  provider_chain: string[]
  max_results: number
  fetch_max_bytes: number
  fetch_max_chars: number
  request_timeout_secs: number
  updated_at: string
}

type ProviderEntry = {
  key: string
  display_name: string
  needs_api_key: boolean
  config_fields: { key: string; label: string; required: boolean; placeholder: string }[]
  configured: boolean
  api_key_set: boolean
  config: Record<string, unknown>
}

function defaultSettings(): Settings {
  return {
    enabled: true,
    provider_chain: ['searxng', 'brave'],
    max_results: 5,
    fetch_max_bytes: 5 * 1024 * 1024,
    fetch_max_chars: 40000,
    request_timeout_secs: 20,
    updated_at: new Date().toISOString(),
  }
}

function defaultCatalog(): ProviderEntry[] {
  return [
    {
      key: 'searxng',
      display_name: 'SearXNG (self-hosted)',
      needs_api_key: false,
      config_fields: [
        { key: 'base_url', label: 'Base URL', required: true, placeholder: 'https://searxng.example.com' },
      ],
      configured: false,
      api_key_set: false,
      config: {},
    },
    {
      key: 'brave',
      display_name: 'Brave Search',
      needs_api_key: true,
      config_fields: [],
      configured: false,
      api_key_set: false,
      config: {},
    },
  ]
}

type State = {
  settings: Settings
  catalog: ProviderEntry[]
  lastSettingsPatch: Partial<Settings> | null
  lastProviderPatch: { provider: string; body: any } | null
}

async function mockApi(page: Page, state: State) {
  await page.route(/\/api\/web-search\/settings$/, async (route, req) => {
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

  await page.route(/\/api\/web-search\/providers$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ providers: state.catalog }),
      })
    }
    return route.continue()
  })

  await page.route(/\/api\/web-search\/providers\/[^/]+$/, async (route, req) => {
    if (req.method() === 'PUT') {
      const provider = req.url().split('/').pop() as string
      const body = JSON.parse(req.postData() ?? '{}')
      state.lastProviderPatch = { provider, body }
      state.catalog = state.catalog.map(p => {
        if (p.key !== provider) return p
        const hasKey = body.api_key !== undefined ? body.api_key.length > 0 : p.api_key_set
        const config = body.config ?? p.config
        const configured = p.needs_api_key ? hasKey : Boolean((config as any).base_url)
        return { ...p, api_key_set: hasKey, config, configured }
      })
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ providers: state.catalog }),
      })
    }
    return route.continue()
  })
}

async function gotoWebSearch(page: Page, baseURL: string) {
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      await page.goto(`${baseURL}/settings/web-search`)
      await expect(page.getByRole('heading', { name: 'Web Search' })).toBeVisible({ timeout: 10000 })
      return
    } catch (e) {
      if (attempt === 3) throw e
      await page.waitForTimeout(1000)
    }
  }
}

const globalSave = (page: Page) =>
  page
    .locator('form')
    .filter({ has: page.getByLabel('Max results per search') })
    .getByRole('button', { name: 'Save' })

const braveSave = (page: Page) =>
  page.locator('form').filter({ has: page.getByLabel('API key') }).getByRole('button', { name: 'Save' })

// ---------------------------------------------------------------------------

test.describe('Web search admin settings', () => {
  test.describe.configure({ retries: 2 })

  test('loads settings + provider chain + provider catalog', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = {
      settings: defaultSettings(),
      catalog: defaultCatalog(),
      lastSettingsPatch: null,
      lastProviderPatch: null,
    }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoWebSearch(page, baseURL)

    await expect(page.getByRole('switch')).toBeChecked()
    await expect(page.getByText('1. SearXNG (self-hosted)')).toBeVisible()
    await expect(page.getByText('2. Brave Search')).toBeVisible()
  })

  test('toggle enabled → Save persists across reload', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = {
      settings: defaultSettings(),
      catalog: defaultCatalog(),
      lastSettingsPatch: null,
      lastProviderPatch: null,
    }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoWebSearch(page, baseURL)

    await page.getByRole('switch').click() // true → false
    await globalSave(page).click()
    await expect(page.getByText('Web search settings saved')).toBeVisible({ timeout: 5000 })
    expect(state.lastSettingsPatch?.enabled).toBe(false)

    await gotoWebSearch(page, baseURL)
    await expect(page.getByRole('switch')).not.toBeChecked()
  })

  test('setting a provider API key never echoes the secret + shows configured', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = {
      settings: defaultSettings(),
      catalog: defaultCatalog(),
      lastSettingsPatch: null,
      lastProviderPatch: null,
    }
    const SECRET = 'BSA-secret-e2e-xyz'
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoWebSearch(page, baseURL)

    await page.getByLabel('API key').fill(SECRET)
    await braveSave(page).click()
    await expect(page.getByText('Brave Search saved')).toBeVisible({ timeout: 5000 })

    expect(state.lastProviderPatch?.provider).toBe('brave')
    expect(state.lastProviderPatch?.body.api_key).toBe(SECRET)
    // The secret must never be rendered back into the DOM.
    await expect(page.getByText(SECRET)).toHaveCount(0)
    // Brave section reflects the configured state after the catalog refresh.
    await expect(page.getByText('Configured').first()).toBeVisible({ timeout: 5000 })
  })

  test('reordering the provider chain persists the new order', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = {
      settings: defaultSettings(),
      catalog: defaultCatalog(),
      lastSettingsPatch: null,
      lastProviderPatch: null,
    }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoWebSearch(page, baseURL)

    // Initially SearXNG is #1, Brave #2.
    await expect(page.getByText('1. SearXNG (self-hosted)')).toBeVisible()

    // The chain editor saves each reorder imperatively (no Save button, no
    // success toast — the visual reorder IS the feedback). Assert the observable
    // effect: the list re-renders from the persisted settings with Brave first.
    await page.getByRole('button', { name: 'Move SearXNG (self-hosted) down' }).click()
    await expect(page.getByText('1. Brave Search')).toBeVisible({ timeout: 5000 })
    await expect(page.getByText('2. SearXNG (self-hosted)')).toBeVisible()
    expect(state.lastSettingsPatch?.provider_chain).toEqual(['brave', 'searxng'])
  })
})
