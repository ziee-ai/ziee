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
    page.getByRole('alert').or(page.getByText(/download.*started|downloading/i)),
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
  const authBadge = modelCard.getByText(/auth.*required/i)
  return await authBadge.isVisible({ timeout: 1000 }).catch(() => false)
}

/**
 * Handle auth required modal by configuring repository
 */
export async function handleAuthRequiredModal(
  page: Page,
  credentials: {
    authType: 'token' | 'username_password'
    token?: string
    username?: string
    password?: string
  },
) {
  // Modal should be visible
  const modal = page.getByRole('dialog', { name: /authentication.*required/i })
  await expect(modal).toBeVisible({ timeout: 2000 })

  // Click configure button
  await modal.getByRole('button', { name: /configure.*authentication/i }).click()

  // Wait for repository settings drawer
  const drawer = page.getByRole('dialog', { name: /repository.*settings|edit.*repository/i })
  await expect(drawer).toBeVisible({ timeout: 3000 })

  // Configure auth
  const authTypeSelect = drawer.getByLabel(/auth.*type/i)
  await authTypeSelect.selectOption({ label: credentials.authType === 'token' ? 'Token' : 'Username/Password' })

  if (credentials.authType === 'token' && credentials.token) {
    await drawer.getByLabel(/token/i).fill(credentials.token)
  } else if (credentials.authType === 'username_password') {
    if (credentials.username) {
      await drawer.getByLabel(/username/i).fill(credentials.username)
    }
    if (credentials.password) {
      await drawer.getByLabel(/password/i).fill(credentials.password)
    }
  }

  // Save settings
  await drawer.getByRole('button', { name: /save|update/i }).click()

  // Wait for drawer to close
  await expect(drawer).not.toBeVisible({ timeout: 3000 })
}
