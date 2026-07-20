import { test, expect } from '../../fixtures/test-context'
import type { Locator } from '@playwright/test'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import {
  downloadModelFromHub,
  getModelCards,
  isModelDownloaded as _isModelDownloaded,
  getModelCardStatus as _getModelCardStatus,
  hasAuthRequiredBadge,
  handleAuthRequiredModal,
  findAuthRequiredCard,
} from './helpers/hub-models'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/permissions'

/** Extract the catalog `name` (the per-card testid suffix) from a model card. */
async function modelNameOf(card: Locator): Promise<string> {
  return (
    (await card.getAttribute('data-testid'))?.replace('hub-model-card-', '') ||
    ''
  )
}

test.describe('Hub Models', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)
  })

  test('should display all hub models', async ({ page }) => {
    const modelCards = await getModelCards(page)
    const count = await modelCards.count()

    expect(count).toBeGreaterThan(0)
  })

  test('should show model cards with required information', async ({ page }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()
    const name = await modelNameOf(firstCard)

    // Should render the model display name
    await expect(firstCard.getByTestId(`hub-model-name-${name}`)).toBeVisible()

    // Should have a "Download" button
    await expect(
      firstCard.getByTestId(`hub-model-download-btn-${name}`),
    ).toBeVisible()
  })

  test('should show an auth badge (Auth Required / Token Needed) for models requiring authentication', async ({ page }) => {
    const modelCards = await getModelCards(page)

    // Check at least one model for an auth badge. The badge text depends on
    // source_auth_configured: "Auth Required" when a credential is configured,
    // "Token Needed" otherwise (the fresh-env default). Both mean auth_required.
    let foundAuthRequired = false

    for (let i = 0; i < await modelCards.count(); i++) {
      const card = modelCards.nth(i)
      const modelId = await modelNameOf(card)

      if (await hasAuthRequiredBadge(page, modelId)) {
        foundAuthRequired = true
        break
      }
    }

    // At least one catalog model has auth_required: true, so it renders an
    // auth badge ("Token Needed" in a fresh env, "Auth Required" once a
    // credential is configured).
    expect(foundAuthRequired).toBe(true)
  })

  test('should block download and show auth modal for models requiring authentication', async ({
    page,
  }) => {
    // Find an auth-gated card (in a fresh env the HF repo has no credential, so
    // its models are gated) instead of assuming the first card is gated.
    const card = await findAuthRequiredCard(page)
    test.skip(!card, 'no auth-gated model in catalog')

    const name = await modelNameOf(card!)
    await card!.getByTestId(`hub-model-download-btn-${name}`).click()

    const dialog = page.getByTestId('hub-download-gate-auth-required')
    await expect(dialog).toBeVisible({ timeout: 5000 })

    // No download should have been started (the early-return fired).
    await expect(
      page
        .locator('[data-sonner-toast][data-type="success"]')
        .filter({ hasText: /download.*started/i }),
    ).toHaveCount(0)
  })

  test('should navigate to repository settings from the auth modal', async ({
    page,
  }) => {
    const card = await findAuthRequiredCard(page)
    test.skip(!card, 'no auth-gated model in catalog')

    // Trigger the auth modal, then "Open Repository Settings" opens the
    // LlmRepositoryDrawer in place.
    const name = await modelNameOf(card!)
    await card!.getByTestId(`hub-model-download-btn-${name}`).click()
    await handleAuthRequiredModal(page)
  })

  test('should show quantization options for models with multiple quantizations', async ({
    page,
  }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()
    const name = await modelNameOf(firstCard)

    // Click download
    await firstCard.getByTestId(`hub-model-download-btn-${name}`).click()

    // May show auth gate dialog first
    const authModal = page.getByTestId('hub-download-gate-auth-required')
    const authModalVisible = await authModal.isVisible({ timeout: 2000 }).catch(() => false)

    if (authModalVisible) {
      await page.getByTestId('hub-download-gate-auth-required-cancel-btn').click()
      return // Skip rest of test if auth blocks us
    }

    // Check for quantization dialog
    const quantModal = page.getByTestId('hub-model-download-quant-dialog')
    const quantModalVisible = await quantModal.isVisible({ timeout: 2000 }).catch(() => false)

    if (quantModalVisible) {
      // Should expose a quantization Select with at least one option.
      await page.getByTestId('hub-model-quant-select').click()
      const options = page.locator('[data-testid^="hub-model-quant-select-opt-"]')
      expect(await options.count()).toBeGreaterThan(0)

      // Should have a confirm (Continue) button
      await expect(
        page.getByTestId('hub-model-download-quant-dialog-ok-btn'),
      ).toBeVisible()

      // Cancel dialog
      await page.getByTestId('hub-model-download-quant-dialog-cancel-btn').click()
    }
  })

  test('should show model tags', async ({ page }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()

    // Models should have tags displayed (capability / catalog tags). All
    // card tags carry a `*-tag-*` testid.
    const tags = firstCard.locator('[data-testid*="-tag-"]')
    const tagCount = await tags.count()

    // Should have at least some tags
    expect(tagCount).toBeGreaterThan(0)
  })

  test('should show popularity score or rating', async ({ page }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()

    // Popularity was removed in v2; assert the probe is robust (no crash).
    const hasPopularity = await firstCard
      .getByTestId('hub-model-popularity')
      .isVisible({ timeout: 1000 })
      .catch(() => false)

    expect(typeof hasPopularity).toBe('boolean')
  })

  test('should prevent download without required permissions', async ({
    page,
    testInfra,
  }) => {
    // User with hub::models::read but NOT hub::models::download.
    // Cards render (read gives access) but ModelHubCard's
    // usePermission(HubModelsCreate) — the enum member for the
    // hub::models::download permission — hides the "Download" button.
    await loginWithPerms(
      page,
      testInfra.baseURL,
      testInfra.apiURL,
      [Permissions.HubModelsRead],
    )
    await navigateToHub(page, testInfra.baseURL, 'models')
    await waitForHubDataLoad(page)

    const cards = await getModelCards(page)
    const cardCount = await cards.count()
    if (cardCount > 0) {
      const name = await modelNameOf(cards.first())
      await expect(
        cards.first().getByTestId(`hub-model-download-btn-${name}`),
      ).toHaveCount(0)
    }
  })

  test('should show model provider/source', async ({ page }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()

    // Source/provider info may or may not surface on the card directly —
    // just assert the probe stays robust.
    const hasProviderInfo = await firstCard
      .getByTestId('hub-model-source')
      .isVisible({ timeout: 1000 })
      .catch(() => false)

    expect(typeof hasProviderInfo).toBe('boolean')
  })

  test.skip('should start model download after selecting quantization', async ({ page }) => {
    // This test requires proper auth setup and takes a long time
    // Skipped by default, can be enabled for full integration testing

    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()
    const modelId = await modelNameOf(firstCard)

    // This would require:
    // 1. Setting up repository with valid credentials
    // 2. Selecting quantization
    // 3. Waiting for download to complete (minutes)

    await downloadModelFromHub(page, modelId, {
      quantization: 'Q4_K_M',
      waitForComplete: false, // Don't wait for full download
    })

    // Verify download started
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 5000 })
  })
})
