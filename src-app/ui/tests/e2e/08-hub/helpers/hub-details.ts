import { Page, expect } from '@playwright/test'

/**
 * Open model details drawer
 */
export async function openModelDetails(page: Page, modelId: string) {
  const modelCard = page.getByTestId(`hub-model-card-${modelId}`)

  // Try clicking the card itself or a "Details" button
  const detailsButton = modelCard.getByRole('button', { name: /details|more/i })
  const hasDetailsButton = await detailsButton.isVisible({ timeout: 1000 }).catch(() => false)

  if (hasDetailsButton) {
    await detailsButton.click()
  } else {
    // Click card title or card itself
    const cardTitle = modelCard.locator('[class*="title"]').first()
    await cardTitle.click()
  }

  // Wait for drawer to open
  const drawer = page.getByRole('dialog', { name: /model.*details/i })
  await expect(drawer).toBeVisible({ timeout: 3000 })

  return drawer
}

/**
 * Open assistant details drawer
 */
export async function openAssistantDetails(page: Page, assistantId: string) {
  const assistantCard = page.getByTestId(`hub-assistant-card-${assistantId}`)

  // Try clicking the card itself or a "Details" button
  const detailsButton = assistantCard.getByRole('button', { name: /details|more/i })
  const hasDetailsButton = await detailsButton.isVisible({ timeout: 1000 }).catch(() => false)

  if (hasDetailsButton) {
    await detailsButton.click()
  } else {
    // Click card title or card itself
    const cardTitle = assistantCard.locator('[class*="title"]').first()
    await cardTitle.click()
  }

  // Wait for drawer to open
  const drawer = page.getByRole('dialog', { name: /assistant.*details/i })
  await expect(drawer).toBeVisible({ timeout: 3000 })

  return drawer
}

/**
 * Open MCP server details drawer
 */
export async function openMcpServerDetails(page: Page, mcpServerId: string) {
  const mcpCard = page.getByTestId(`hub-mcp-card-${mcpServerId}`)

  // Try clicking the card itself or a "Details" button
  const detailsButton = mcpCard.getByRole('button', { name: /details|more/i })
  const hasDetailsButton = await detailsButton.isVisible({ timeout: 1000 }).catch(() => false)

  if (hasDetailsButton) {
    await detailsButton.click()
  } else {
    // Click card title or card itself
    const cardTitle = mcpCard.locator('[class*="title"]').first()
    await cardTitle.click()
  }

  // Wait for drawer to open
  const drawer = page.getByRole('dialog', { name: /mcp.*details|server.*details/i })
  await expect(drawer).toBeVisible({ timeout: 3000 })

  return drawer
}

/**
 * Close details drawer
 */
export async function closeDetailsDrawer(page: Page) {
  const drawer = page.getByRole('dialog')
  const closeButton = drawer.getByRole('button', { name: /close/i }).or(
    drawer.locator('[aria-label*="close" i]')
  )

  await closeButton.click()
  await expect(drawer).not.toBeVisible({ timeout: 2000 })
}

/**
 * Get detail field value from drawer
 */
export async function getDetailFieldValue(
  page: Page,
  fieldLabel: string,
): Promise<string | null> {
  const drawer = page.getByRole('dialog')

  // Try to find label + value pattern
  const field = drawer.getByText(new RegExp(`${fieldLabel}:?`, 'i'))
  const visible = await field.isVisible({ timeout: 1000 }).catch(() => false)

  if (visible) {
    // Get the next sibling or parent's next element
    const parent = field.locator('..')
    const value = parent.locator('~ *').first()
    const valueText = await value.textContent().catch(() => null)
    return valueText?.trim() || null
  }

  return null
}

/**
 * Check if details drawer has specific tag
 */
export async function detailsHasTag(page: Page, tagText: string): Promise<boolean> {
  const drawer = page.getByRole('dialog')
  const tag = drawer.getByText(new RegExp(tagText, 'i'))
  return await tag.isVisible({ timeout: 1000 }).catch(() => false)
}
