import { Page, expect } from '@playwright/test'

/**
 * Create assistant from hub template
 */
export async function createAssistantFromHub(
  page: Page,
  assistantId: string,
  customize?: {
    name?: string
    description?: string
    instructions?: string
  },
) {
  const assistantCard = page.getByTestId(`hub-assistant-card-${assistantId}`)
  await assistantCard.getByRole('button', { name: /use/i }).click()

  // Handle customization modal if it appears
  if (customize) {
    const modal = page.getByRole('dialog', { name: /customize/i })
    const modalVisible = await modal.isVisible({ timeout: 2000 }).catch(() => false)

    if (modalVisible) {
      if (customize.name) {
        await modal.getByLabel(/name/i).fill(customize.name)
      }
      if (customize.description) {
        await modal.getByLabel(/description/i).fill(customize.description)
      }
      if (customize.instructions) {
        await modal.getByLabel(/instructions/i).fill(customize.instructions)
      }
      await modal.getByRole('button', { name: /create/i }).click()
    }
  }

  // Wait for success message or navigation
  await expect(
    page.getByRole('alert').or(page.getByText(/created.*successfully/i)),
  ).toBeVisible({ timeout: 5000 })
}

/**
 * Get assistant card status badge
 */
export async function getAssistantCardStatus(
  page: Page,
  assistantId: string,
): Promise<string | null> {
  const assistantCard = page.getByTestId(`hub-assistant-card-${assistantId}`)
  const badge = assistantCard.getByText(/created/i)

  const visible = await badge.isVisible({ timeout: 1000 }).catch(() => false)
  if (visible) {
    return await badge.textContent()
  }

  return null
}

/**
 * Check if assistant has "View" button (indicating it's been created)
 */
export async function isAssistantCreated(
  page: Page,
  assistantId: string,
): Promise<boolean> {
  const assistantCard = page.getByTestId(`hub-assistant-card-${assistantId}`)
  const viewButton = assistantCard.getByRole('button', { name: /view/i })
  return await viewButton.isVisible({ timeout: 1000 }).catch(() => false)
}

/**
 * Get all assistant cards
 */
export async function getAssistantCards(page: Page) {
  return page.getByTestId(/^hub-assistant-card-/)
}
