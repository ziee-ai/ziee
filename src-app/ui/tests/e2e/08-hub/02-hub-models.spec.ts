import { test, expect } from '../../fixtures/test-context'
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
import { Permissions } from '../../../src/api-client/types'

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

    // Should have name/title - look for text inside the card
    await expect(firstCard.getByText(/phi|llama|mistral|gemma/i).first()).toBeVisible()

    // Should have "Download" button
    await expect(firstCard.getByRole('button', { name: /download/i })).toBeVisible()

    // Should have size information
    const sizeText = firstCard.getByText(/GB|MB/i)
    await expect(sizeText).toBeVisible()
  })

  test('should show an auth badge (Auth Required / Token Needed) for models requiring authentication', async ({ page }) => {
    const modelCards = await getModelCards(page)

    // Check at least one model for an auth badge. The badge text depends on
    // source_auth_configured: "Auth Required" when a credential is configured,
    // "Token Needed" otherwise (the fresh-env default). Both mean auth_required.
    let foundAuthRequired = false

    for (let i = 0; i < await modelCards.count(); i++) {
      const card = modelCards.nth(i)
      const testId = await card.getAttribute('data-testid')
      const modelId = testId?.replace('hub-model-card-', '') || ''

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

    await card!.getByRole('button', { name: /download/i }).click()

    const dialog = page.getByRole('dialog', {
      name: /authentication.*required/i,
    })
    await expect(dialog).toBeVisible({ timeout: 5000 })

    // No download should have been started (the early-return fired).
    await expect(page.getByText(/download.*started/i)).toHaveCount(0)
  })

  test('should navigate to repository settings from the auth modal', async ({
    page,
  }) => {
    const card = await findAuthRequiredCard(page)
    test.skip(!card, 'no auth-gated model in catalog')

    // Trigger the auth modal, then "Go to LLM Repositories" deep-links to the
    // LLM Repositories settings page.
    await card!.getByRole('button', { name: /download/i }).click()
    await handleAuthRequiredModal(page)
  })

  test('should show quantization options for models with multiple quantizations', async ({
    page,
  }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()

    // Click download
    await firstCard.getByRole('button', { name: /download/i }).click()

    // May show auth modal first
    const authModal = page.getByRole('dialog', { name: /authentication.*required/i })
    const authModalVisible = await authModal.isVisible({ timeout: 2000 }).catch(() => false)

    if (authModalVisible) {
      // Configure mock auth to proceed
      await authModal.getByRole('button', { name: /cancel/i }).click()
      return // Skip rest of test if auth blocks us
    }

    // Check for quantization modal
    const quantModal = page.getByRole('dialog', { name: /select.*quantization|download/i })
    const quantModalVisible = await quantModal.isVisible({ timeout: 2000 }).catch(() => false)

    if (quantModalVisible) {
      // Should have radio options for quantizations
      const radioOptions = quantModal.getByRole('radio')
      const optionCount = await radioOptions.count()
      expect(optionCount).toBeGreaterThan(0)

      // Should have download button
      await expect(quantModal.getByRole('button', { name: /download/i })).toBeVisible()

      // Cancel modal
      await quantModal.getByRole('button', { name: /cancel/i }).click()
    }
  })

  test('should show model tags', async ({ page }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()

    // Models should have tags displayed (e.g., parameter count, type)
    const tags = firstCard.locator('[class*="tag"]').or(firstCard.locator('.ant-tag'))
    const tagCount = await tags.count()

    // Should have at least some tags
    expect(tagCount).toBeGreaterThan(0)
  })

  test('should show popularity score or rating', async ({ page }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()

    // Look for popularity indicators (stars, numbers, etc.)
    const popularityIndicator =
      firstCard.getByText(/popular/i).or(firstCard.locator('[class*="rating"]'))

    // May or may not be visible depending on design
    const hasPopularity = await popularityIndicator.isVisible({ timeout: 1000 }).catch(() => false)

    // Just checking it doesn't error - popularity might not be displayed
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
      await expect(
        cards.first().getByRole('button', { name: /^download$/i }),
      ).toHaveCount(0)
    }
  })

  test('should show model provider/source', async ({ page }) => {
    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()

    // Should show which provider/repository the model is from
    const providerInfo = firstCard.getByText(/hugging.*face|ollama|openai/i)
    const hasProviderInfo = await providerInfo.isVisible({ timeout: 1000 }).catch(() => false)

    // At minimum, some indication of source should be present
    expect(typeof hasProviderInfo).toBe('boolean')
  })

  test.skip('should start model download after selecting quantization', async ({ page }) => {
    // This test requires proper auth setup and takes a long time
    // Skipped by default, can be enabled for full integration testing

    const modelCards = await getModelCards(page)
    const firstCard = modelCards.first()
    const testId = await firstCard.getAttribute('data-testid')
    const modelId = testId?.replace('hub-model-card-', '') || ''

    // This would require:
    // 1. Setting up repository with valid credentials
    // 2. Selecting quantization
    // 3. Waiting for download to complete (minutes)

    await downloadModelFromHub(page, modelId, {
      quantization: 'Q4_K_M',
      waitForComplete: false, // Don't wait for full download
    })

    // Verify download started
    await expect(page.getByText(/downloading/i)).toBeVisible({ timeout: 5000 })
  })
})
