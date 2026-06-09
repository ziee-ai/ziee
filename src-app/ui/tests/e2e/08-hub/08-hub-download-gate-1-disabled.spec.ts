/**
 * Hub model download gate — REPOSITORY DISABLED.
 *
 * When the source repository is disabled, clicking Download shows the
 * "Repository Disabled" modal whose primary button opens the
 * LlmRepositoryDrawer for that repo. NO download POST is fired.
 */

import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { getModelCards } from './helpers/hub-models'

async function disableHuggingFaceRepo(
  apiURL: string,
  token: string,
): Promise<string> {
  // Find the HF repo, disable it via the public API.
  const list = await fetch(`${apiURL}/api/llm-repositories`, {
    headers: { Authorization: `Bearer ${token}` },
  }).then(r => r.json())
  const hf = (list.repositories as any[]).find(
    r => r.name === 'Hugging Face Hub',
  )
  if (!hf) throw new Error('Hugging Face Hub repository not found in seed')
  const resp = await fetch(`${apiURL}/api/llm-repositories/${hf.id}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ enabled: false }),
  })
  if (!resp.ok) {
    throw new Error(`disable repo failed: ${resp.status} ${await resp.text()}`)
  }
  return hf.id
}

/** Count download-start POSTs so we can assert ZERO on the gate fail. */
async function trackDownloadStartCount(page: Page): Promise<() => number> {
  let hits = 0
  await page.route(/\/api\/hub\/models\/download$/, async (route, request) => {
    if (request.method() === 'POST') hits++
    return route.fallback()
  })
  return () => hits
}

test('clicking Download on a disabled-repo model shows the disabled modal + opens the drawer', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  const token = await getAdminToken(baseURL)
  await disableHuggingFaceRepo(baseURL, token)

  const getHits = await trackDownloadStartCount(page)

  await loginAsAdmin(page, baseURL)
  await navigateToHub(page, baseURL, 'models')
  await waitForHubDataLoad(page)

  // Click Download on the first model card (catalog HF-hosted; same
  // repo we just disabled).
  const firstCard = (await getModelCards(page)).first()
  await expect(firstCard).toBeVisible()
  await firstCard.getByRole('button', { name: /download/i }).click()

  // "Repository Disabled" modal appears. The probe never runs because
  // the enabled-gate fires first.
  const modal = page.getByRole('dialog').filter({ hasText: 'Repository Disabled' })
  await expect(modal).toBeVisible({ timeout: 10_000 })

  // Primary button → opens the LlmRepositoryDrawer. The drawer's
  // title contains "Edit" since the disabled HF repo is built-in.
  await modal.getByRole('button', { name: 'Open Repository Settings' }).click()
  await expect(
    page.locator('.ant-drawer.ant-drawer-open .ant-drawer-title').last(),
  ).toContainText(/Built-in Repository/, { timeout: 10_000 })

  // NO download POST was fired.
  expect(getHits()).toBe(0)
})
