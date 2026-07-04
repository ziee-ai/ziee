import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToProvidersPage } from './helpers/navigation-helpers'

/**
 * E2E — LLM-provider list ordering (sortProviders: Local first, then the rest
 * alphabetically by name).
 *
 * Audit gap: the canonical provider ordering applied to the settings list was
 * never asserted through the UI. This creates providers OUT of order (a local
 * named 'ZZZ' + two remotes 'AAA'/'MMM') and asserts the rendered menu order
 * is [ZZZ Local, AAA Remote, MMM Remote] — proving local-first beats alpha and
 * the remotes are alphabetised.
 */

async function createProvider(
  request: import('@playwright/test').APIRequestContext,
  apiURL: string,
  token: string,
  name: string,
  type: 'local' | 'openai',
): Promise<void> {
  const res = await request.post(`${apiURL}/api/llm-providers`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      name,
      provider_type: type,
      enabled: false,
      ...(type === 'openai' ? { api_key: 'sk-test123' } : {}),
    },
  })
  expect(res.status(), `create ${name}: ${await res.text()}`).toBe(201)
}

async function y(page: import('@playwright/test').Page, name: string) {
  // The provider list renders as vertical nav buttons (kit Menu is a <nav>,
  // not role="menu"); each item carries a `llm-provider-nav-<id>` testid.
  const item = page
    .locator('[data-testid^="llm-provider-nav-"]')
    .filter({ hasText: name })
    .first()
  await item.waitFor({ state: 'visible', timeout: 15000 })
  const box = await item.boundingBox()
  if (!box) throw new Error(`no bounding box for ${name}`)
  return box.y
}

test.describe('LLM providers — list ordering', () => {
  test('local provider sorts first, remotes alphabetical', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(
      () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )

    const tag = Date.now().toString(36)
    const localName = `ZZZ Local ${tag}`
    const aaaName = `AAA Remote ${tag}`
    const mmmName = `MMM Remote ${tag}`
    // Create in a deliberately unsorted order.
    await createProvider(request, apiURL, token, mmmName, 'openai')
    await createProvider(request, apiURL, token, localName, 'local')
    await createProvider(request, apiURL, token, aaaName, 'openai')

    await goToProvidersPage(page, baseURL)

    const yLocal = await y(page, localName)
    const yAaa = await y(page, aaaName)
    const yMmm = await y(page, mmmName)

    // Local first (despite the 'ZZZ' name), then remotes alphabetically.
    expect(yLocal).toBeLessThan(yAaa)
    expect(yAaa).toBeLessThan(yMmm)
  })
})
