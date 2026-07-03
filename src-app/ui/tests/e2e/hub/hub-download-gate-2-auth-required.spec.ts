/**
 * Hub model download gate — AUTHENTICATION REQUIRED.
 *
 * When the repo is enabled but the model needs auth AND no credential
 * is configured, the connection probe fails and we show the
 * "Authentication Required" modal. Primary button opens the
 * LlmRepositoryDrawer for that repo. NO download POST is fired.
 *
 * Replaces the prior modal that navigated to the LLM Repositories
 * settings page (per the user request — open the drawer directly).
 */

import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * Mock the by-id test endpoint so it deterministically returns 401-style
 * failure WITHOUT having to actually hit huggingface.co. Sidesteps
 * network flakiness in CI.
 */
async function mockRepoTestById(
  page: Page,
  outcome: 'fail',
): Promise<() => number> {
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
          success: outcome !== 'fail',
          message: outcome === 'fail'
            ? 'Connection to Hugging Face Hub failed: 401 Unauthorized'
            : 'Connection successful',
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

/** The HF seed repo is enabled by default + has no credential. Find an
 *  auth-required hub model whose card is actually RENDERED (some catalog
 *  entries — e.g. very large models with no downloadable variant — aren't
 *  shown on the hub page) so we can click its Download button.
 *
 *  v2 Phase 7: there is no model-wide `auth_required` flag anymore; the
 *  need for auth is derived from a source's `environmentVariables[]`
 *  entry that is `isRequired && isSecret` (see ModelHubCard.tsx). The
 *  card testid is keyed on `model.name` (reverse-DNS), not an `id`.
 *  The hub page must already be open. */
async function firstAuthRequiredHubModel(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
): Promise<{ name: string; display_name: string }> {
  const body = await fetch(`${apiURL}/api/hub/models?lang=en`, {
    headers: { Authorization: `Bearer ${token}` },
  }).then(r => r.json())
  const needsAuth = (m: any): boolean =>
    Array.isArray(m.sources) &&
    m.sources.some((s: any) =>
      (s.environmentVariables ?? []).some(
        (e: any) => e.isRequired === true && e.isSecret === true,
      ),
    )
  for (const m of (body as any[]).filter(needsAuth)) {
    const card = page.locator(`[data-testid="hub-model-card-${m.name}"]`)
    if (await card.isVisible().catch(() => false)) {
      return { name: m.name, display_name: m.display_name }
    }
  }
  throw new Error('no rendered auth-required hub model in catalog')
}

test('Download on an auth-required model with no credential shows the auth-required modal + opens the drawer', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  const token = await getAdminToken(baseURL)
  // HF seed is enabled by default; we DON'T configure a token here so
  // model.auth_required && !model.source_auth_configured will hold.

  const probeHits = await mockRepoTestById(page, 'fail')
  const dlHits = await trackDownloadStartCount(page)

  await loginAsAdmin(page, baseURL)
  await navigateToHub(page, baseURL, 'models')
  await waitForHubDataLoad(page)

  // Pick an auth-required model whose card is rendered on the hub page.
  const targetModel = await firstAuthRequiredHubModel(page, baseURL, token)

  // Find that specific model's card. The card uses
  // data-testid="hub-model-card-<name>" (per ModelHubCard.tsx) so we can
  // target it directly even if the catalog has many entries.
  const card = page.getByTestId(`hub-model-card-${targetModel.name}`)
  await expect(card).toBeVisible({ timeout: 10_000 })
  await card.getByTestId(`hub-model-download-btn-${targetModel.name}`).click()

  // The probe runs (mock returns failure). Auth-required branch fires.
  const modal = page.getByTestId('hub-download-gate-auth-required')
  await expect(modal).toBeVisible({ timeout: 15_000 })
  expect(probeHits()).toBeGreaterThanOrEqual(1)

  await page.getByTestId('hub-download-gate-auth-required-ok-btn').click()
  await expect(page.getByTestId('llmrepo-form')).toBeVisible({
    timeout: 10_000,
  })

  expect(dlHits()).toBe(0)
})
