import { Page, expect } from '@playwright/test'

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
  await modelCard.getByRole('button', { name: /download/i }).click()

  // Handle quantization selection modal if it appears
  const modal = page.getByRole('dialog', { name: /select.*quantization|download/i })
  const modalVisible = await modal.isVisible({ timeout: 2000 }).catch(() => false)

  if (modalVisible && options?.quantization) {
    await modal.getByRole('radio', { name: new RegExp(options.quantization, 'i') }).click()
    await modal.getByRole('button', { name: /download/i }).click()
  }

  // Wait for download to start
  await expect(
    page.getByRole('alert').or(page.getByText(/download.*started|downloading/i)).first(),
  ).toBeVisible({ timeout: 5000 })

  // Optionally wait for download to complete
  if (options?.waitForComplete) {
    await expect(page.getByText(/download.*completed|ready/i)).toBeVisible({
      timeout: 300000, // 5 minutes for model downloads
    })
  }
}

/**
 * Get model card download status
 */
export async function getModelCardStatus(
  page: Page,
  modelId: string,
): Promise<string | null> {
  const modelCard = page.getByTestId(`hub-model-card-${modelId}`)
  const badge = modelCard.locator('[class*="status"]').or(modelCard.getByText(/downloaded|downloading/i))

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
  const modelCard = page.getByTestId(`hub-model-card-${modelId}`)
  const viewButton = modelCard.getByRole('button', { name: /view/i })
  return await viewButton.isVisible({ timeout: 1000 }).catch(() => false)
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
  const modelCard = page.getByTestId(`hub-model-card-${modelId}`)
  // The badge reads "Auth Required" once the source repo has a credential and
  // "Token Needed" while it does not — both mean authentication is required.
  const authBadge = modelCard.getByText(/auth.*required|token needed/i)
  return await authBadge.isVisible({ timeout: 1000 }).catch(() => false)
}

/**
 * Find the first model card that shows an auth badge (Auth Required / Token
 * Needed) — i.e. an auth-gated model. Returns the card locator, or null if none.
 * Use this instead of `.first()` so tests don't depend on catalog ordering.
 */
export async function findAuthRequiredCard(page: Page) {
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
 * Handle the "Authentication Required" modal: assert it is shown, click
 * "Go to LLM Repositories", and assert it deep-links to the LLM Repositories
 * settings page (the hub module guides the user there rather than embedding the
 * repository editor — a module-boundary-clean design).
 */
export async function handleAuthRequiredModal(page: Page) {
  const modal = page.getByRole('dialog', { name: /authentication.*required/i })
  await expect(modal).toBeVisible({ timeout: 5000 })

  await modal.getByRole('button', { name: /go to llm repositories/i }).click()

  await expect(page).toHaveURL(/\/settings\/llm-repositories/)
}
