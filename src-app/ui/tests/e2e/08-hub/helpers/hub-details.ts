import { Page, Locator, expect } from '@playwright/test'

/**
 * Open model details drawer. The card root opens the details drawer
 * (inner action buttons stopPropagation).
 */
export async function openModelDetails(page: Page, modelId: string): Promise<Locator> {
  await page.getByTestId(`hub-model-card-${modelId}`).click()
  const drawer = page.getByTestId('hub-model-detail-sheet')
  await expect(drawer).toBeVisible({ timeout: 3000 })
  return drawer
}

/**
 * Open assistant details drawer (via the card's Details button).
 */
export async function openAssistantDetails(
  page: Page,
  assistantId: string,
): Promise<Locator> {
  await page
    .getByTestId(`hub-assistant-card-${assistantId}`)
    .getByTestId(`hub-assistant-details-btn-${assistantId}`)
    .click()
  const drawer = page.getByTestId('hub-assistant-detail-sheet')
  await expect(drawer).toBeVisible({ timeout: 3000 })
  return drawer
}

/**
 * Open MCP server details drawer (via the card root).
 */
export async function openMcpServerDetails(
  page: Page,
  mcpServerId: string,
): Promise<Locator> {
  await page.getByTestId(`hub-mcp-card-${mcpServerId}`).click()
  const drawer = page.getByTestId('hub-mcp-detail-sheet')
  await expect(drawer).toBeVisible({ timeout: 3000 })
  return drawer
}

/**
 * Close the currently open details drawer/sheet (Radix dialogs close on Escape).
 */
export async function closeDetailsDrawer(page: Page) {
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('hub-assistant-detail-sheet')).toBeHidden({
    timeout: 2000,
  }).catch(() => {})
}

/**
 * Check whether a known detail card/tag (by testid) is present in an open drawer.
 */
export async function detailsHasTestId(page: Page, testId: string): Promise<boolean> {
  return await page
    .getByTestId(testId)
    .isVisible({ timeout: 1000 })
    .catch(() => false)
}
