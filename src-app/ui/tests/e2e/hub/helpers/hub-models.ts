import { Page, expect, Locator } from '@playwright/test'

/**
 * Download model from hub
 */
export async function downloadModelFromHub(
  page: Page,
  modelId: string,
  options?: {
    quantization?: string
    waitForComplete?: boolean
  },
) {
  const modelCard = page.getByTestId(`hub-model-card-${modelId}`)
  await modelCard.getByTestId(`hub-model-download-btn-${modelId}`).click()

  // Quantization-selection dialog only appears for multi-quant sources.
  const quantDialog = page.getByTestId('hub-model-download-quant-dialog')
  const quantVisible = await quantDialog
    .isVisible({ timeout: 2000 })
    .catch(() => false)

  if (quantVisible && options?.quantization) {
    await page.getByTestId('hub-model-quant-select').click()
    await page
      .getByTestId(`hub-model-quant-select-opt-${options.quantization}`)
      .click()
    await page.getByTestId('hub-model-download-quant-dialog-ok-btn').click()
  }

  // A provider-selection dialog may follow when multiple local providers exist.
  const providerDialog = page.getByTestId('hub-model-download-provider-dialog')
  if (await providerDialog.isVisible({ timeout: 1000 }).catch(() => false)) {
    await page.getByTestId('hub-model-download-provider-dialog-ok-btn').click()
  }

  // Download-started success toast.
  await expect(
    page.locator('[data-sonner-toast][data-type="success"]').first(),
  ).toBeVisible({ timeout: 5000 })

  // Optionally wait for download to complete — the card flips to a
  // "Downloaded" status tag.
  if (options?.waitForComplete) {
    await expect(
      page.getByTestId(`hub-model-status-tag-${modelId}`),
    ).toBeVisible({ timeout: 300000 })
  }
}

/**
 * Get model card download status tag text (or null when absent).
 */
export async function getModelCardStatus(
  page: Page,
  modelId: string,
): Promise<string | null> {
  const badge = page.getByTestId(`hub-model-status-tag-${modelId}`)

  const visible = await badge.isVisible({ timeout: 1000 }).catch(() => false)
  if (visible) {
    return await badge.textContent()
  }

  return null
}

/**
 * Check if model has "View" button (indicating it's been downloaded)
 */
export async function isModelDownloaded(
  page: Page,
  modelId: string,
): Promise<boolean> {
  const badge = page.getByTestId(`hub-model-status-tag-${modelId}`)
  return await badge.isVisible({ timeout: 1000 }).catch(() => false)
}

/**
 * Get all model cards
 */
export async function getModelCards(page: Page) {
  return page.getByTestId(/^hub-model-card-/)
}

/**
 * Check if model has auth required badge
 */
export async function hasAuthRequiredBadge(
  page: Page,
  modelId: string,
): Promise<boolean> {
  const authBadge = page.getByTestId(`hub-model-auth-tag-${modelId}`)
  return await authBadge.isVisible({ timeout: 1000 }).catch(() => false)
}

/**
 * Find the first model card that shows an auth badge (auth-gated model).
 * Returns the card locator, or null if none. Avoids depending on catalog order.
 */
export async function findAuthRequiredCard(page: Page): Promise<Locator | null> {
  const cards = page.getByTestId(/^hub-model-card-/)
  const n = await cards.count()
  for (let i = 0; i < n; i++) {
    const card = cards.nth(i)
    const id =
      (await card.getAttribute('data-testid'))?.replace('hub-model-card-', '') ||
      ''
    if (await hasAuthRequiredBadge(page, id)) return card
  }
  return null
}

/**
 * Handle the "Authentication Required" gate dialog: assert it is shown, click
 * "Open Repository Settings", and assert the LlmRepositoryDrawer opened in place.
 */
export async function handleAuthRequiredModal(page: Page) {
  const modal = page.getByTestId('hub-download-gate-auth-required')
  await expect(modal).toBeVisible({ timeout: 5000 })

  await page.getByTestId('hub-download-gate-auth-required-ok-btn').click()

  await expect(page.getByTestId('llmrepo-form')).toBeVisible({ timeout: 5000 })
}
