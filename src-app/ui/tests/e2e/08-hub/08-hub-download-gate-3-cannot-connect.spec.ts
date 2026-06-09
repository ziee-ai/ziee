/**
 * Hub model download gate — CANNOT CONNECT.
 *
 * The repo is enabled AND a credential is configured for it, but the
 * connection probe still fails (bad token / unreachable upstream).
 * Shows the "Cannot Connect to Repository" modal whose primary button
 * opens the LlmRepositoryDrawer. NO download POST is fired.
 */

import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { getModelCards } from './helpers/hub-models'

async function configureHfWithDummyKey(
  apiURL: string,
  token: string,
): Promise<string> {
  const list = await fetch(`${apiURL}/api/llm-repositories`, {
    headers: { Authorization: `Bearer ${token}` },
  }).then(r => r.json())
  const hf = (list.repositories as any[]).find(
    r => r.name === 'Hugging Face Hub',
  )
  if (!hf) throw new Error('Hugging Face Hub repository not found in seed')
  // A dummy (non-empty) api_key flips the computed
  // `source_auth_configured` to true on the catalog response, so the
  // auth-required branch in handleDownload is skipped and we exercise
  // the cannot-connect branch instead.
  const resp = await fetch(`${apiURL}/api/llm-repositories/${hf.id}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      auth_config: { api_key: 'hf-dummy-key-for-e2e' },
    }),
  })
  if (!resp.ok) {
    throw new Error(
      `configure HF api_key failed: ${resp.status} ${await resp.text()}`,
    )
  }
  return hf.id
}

async function mockRepoTestByIdFail(page: Page): Promise<() => number> {
  let hits = 0
  await page.route(
    /\/api\/llm-repositories\/[^/]+\/test$/,
    async (route, request) => {
      if (request.method() !== 'POST') return route.fallback()
      hits++
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          success: false,
          message:
            'Connection to Hugging Face Hub failed: HTTP request failed with status: 503',
        }),
      })
    },
  )
  return () => hits
}

async function trackDownloadStartCount(page: Page): Promise<() => number> {
  let hits = 0
  await page.route(/\/api\/hub\/models\/download$/, async (route, request) => {
    if (request.method() === 'POST') hits++
    return route.fallback()
  })
  return () => hits
}

test('Download on a credentialed-but-unreachable repo shows the cannot-connect modal + opens the drawer', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  const token = await getAdminToken(baseURL)
  await configureHfWithDummyKey(baseURL, token)

  const probeHits = await mockRepoTestByIdFail(page)
  const dlHits = await trackDownloadStartCount(page)

  await loginAsAdmin(page, baseURL)
  await navigateToHub(page, baseURL, 'models')
  await waitForHubDataLoad(page)

  // First model card works — repo is the HF seed which is now
  // credentialed (dummy key) + enabled. The probe mock fails the
  // probe with a non-auth-shaped error so the cannot-connect branch
  // fires regardless of the model's auth_required flag.
  const firstCard = (await getModelCards(page)).first()
  await expect(firstCard).toBeVisible()
  await firstCard.getByRole('button', { name: /download/i }).click()

  const modal = page
    .getByRole('dialog')
    .filter({ hasText: 'Cannot Connect to Repository' })
  await expect(modal).toBeVisible({ timeout: 15_000 })
  expect(probeHits()).toBeGreaterThanOrEqual(1)

  await modal.getByRole('button', { name: 'Open Repository Settings' }).click()
  await expect(
    page.locator('.ant-drawer.ant-drawer-open .ant-drawer-title').last(),
  ).toContainText(/Built-in Repository/, { timeout: 10_000 })

  expect(dlHits()).toBe(0)
})
