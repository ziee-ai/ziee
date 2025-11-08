import { Page, expect } from '@playwright/test'

/**
 * Navigation helpers for assistants pages
 *
 * Uses semantic selectors following CLAUDE.md best practices
 */

export async function goToUserAssistantsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/assistants`)
  await page.waitForLoadState('networkidle')
  // Wait for the page title (h4 heading in title bar) specifically, not the empty state heading (h3)
  await page.getByRole('heading', { level: 4, name: /assistants/i }).first().waitFor({ timeout: 10000 })
}

export async function goToTemplateAssistantsSettings(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/assistants`)
  await page.waitForLoadState('networkidle')
  await page.getByText('Template Assistants').or(page.locator('.ant-card-head-title:has-text("Template Assistants")')).waitFor({ timeout: 10000 })
}

/**
 * Assistant Form Drawer helpers
 */

export async function openCreateAssistantDrawer(page: Page, _isUserPage = true) {
  // Both pages now use the same aria-label for the create button
  await page.getByRole('button', { name: /create assistant/i }).click()
  await page.getByRole('dialog').or(page.locator('.ant-drawer')).waitFor({ state: 'visible' })
}

export async function fillAssistantForm(
  page: Page,
  data: {
    name: string
    description?: string
    instructions?: string
    parameters?: string
    enabled?: boolean
    isDefault?: boolean
  }
) {
  // Wait for drawer to be fully loaded by waiting for first input field
  await page.getByLabel('Name').waitFor({ state: 'visible' })

  // Fill name
  await page.getByLabel('Name').fill(data.name)

  // Fill description if provided
  if (data.description !== undefined) {
    await page.getByLabel('Description').fill(data.description)
  }

  // Fill instructions if provided
  if (data.instructions !== undefined) {
    await page.getByLabel('Instructions').fill(data.instructions)
  }

  // Fill parameters if provided
  if (data.parameters !== undefined) {
    await page.getByLabel('Parameters').fill(data.parameters)
  }

  // Set enabled toggle
  if (data.enabled !== undefined) {
    // Use semantic selector with fallback to ID
    const enabledSwitch = page.getByLabel('Enabled').or(page.locator('#assistant-form_enabled'))
    await enabledSwitch.waitFor({ state: 'visible', timeout: 5000 })
    const isEnabled = await enabledSwitch.getAttribute('aria-checked')
    if ((isEnabled === 'true') !== data.enabled) {
      await enabledSwitch.click()
    }
  }

  // Set default toggle
  if (data.isDefault !== undefined) {
    // Use semantic selector with fallback to ID
    const defaultSwitch = page.getByLabel(/default/i).or(page.locator('#assistant-form_is_default'))
    await defaultSwitch.waitFor({ state: 'visible', timeout: 5000 })
    const isDefault = await defaultSwitch.getAttribute('aria-checked')
    if ((isDefault === 'true') !== data.isDefault) {
      await defaultSwitch.click()
    }
  }
}

export async function submitAssistantForm(page: Page) {
  await page.getByRole('dialog').or(page.locator('.ant-drawer')).getByRole('button', { name: /submit|create|save/i }).click()
  // Don't wait for drawer here - let the test verify success message first
  // The drawer will close automatically after successful submission
}

export async function cancelAssistantForm(page: Page) {
  // Find the Cancel button within any visible drawer
  await page.getByRole('dialog').or(page.locator('.ant-drawer')).getByRole('button', { name: 'Cancel' }).click()

  // Wait for the drawer to close by checking that no visible drawers remain
  // We check the drawer wrapper class that Ant Design uses when drawer is open
  await page.locator('.ant-drawer.ant-drawer-open').waitFor({
    state: 'hidden',
    timeout: 10000
  })
}

/**
 * Assistant Card helpers (User Assistants Page)
 */

export async function getAssistantCard(page: Page, assistantName: string) {
  return page.locator(`[data-test-assistant-name="${assistantName}"]`)
}

export async function editAssistantFromCard(page: Page, assistantName: string) {
  const card = await getAssistantCard(page, assistantName)

  // Click the menu button (last button with an SVG)
  await card.locator('button:has(svg)').last().click()

  // Wait for dropdown menu
  await page.getByRole('menu').or(page.locator('.ant-dropdown-menu')).waitFor({ state: 'visible' })

  // Click Edit
  await page.getByRole('menuitem', { name: 'Edit' }).or(page.locator('.ant-dropdown-menu').getByText('Edit', { exact: true })).click()

  // Wait for drawer to open
  await page.getByRole('dialog').or(page.locator('.ant-drawer')).waitFor({ state: 'visible' })

  // Wait for form content to be loaded (same as fillAssistantForm does)
  await page.getByLabel('Name').waitFor({ state: 'visible', timeout: 10000 })
}

export async function deleteAssistantFromCard(page: Page, assistantName: string) {
  const card = await getAssistantCard(page, assistantName)

  // Click the menu button
  await card.locator('button:has(svg)').last().click()

  // Wait for dropdown menu
  await page.getByRole('menu').or(page.locator('.ant-dropdown-menu')).waitFor({ state: 'visible' })

  // Click Delete
  await page.getByRole('menuitem', { name: 'Delete' }).or(page.locator('.ant-dropdown-menu').getByText('Delete', { exact: true })).click()

  // Confirm deletion in modal
  await page.getByRole('dialog').or(page.locator('.ant-modal')).waitFor({ state: 'visible' })
  await page.getByRole('dialog').or(page.locator('.ant-modal')).getByRole('button', { name: 'Delete' }).click()

  // Wait for modal to close
  await page.getByRole('dialog').or(page.locator('.ant-modal')).waitFor({ state: 'hidden' })
}

export async function clickAssistantCard(page: Page, assistantName: string) {
  const card = await getAssistantCard(page, assistantName)
  await card.click()
  await page.getByRole('dialog').or(page.locator('.ant-drawer')).waitFor({ state: 'visible' })
}

/**
 * Template Assistant List helpers (Settings Page)
 */

export async function getTemplateAssistantRow(page: Page, assistantName: string) {
  // Find the template assistant row by data-test-assistant-id and text content
  // Use >> to drill down: find divs with test ID that contain the exact text
  return page.locator(`[data-test-assistant-id^="template-assistant-"]:has-text("${assistantName}")`)
}

export async function editTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  await row.getByRole('button', { name: 'Edit' }).click()
  await page.getByRole('dialog').or(page.locator('.ant-drawer')).waitFor({ state: 'visible' })
}

export async function deleteTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  await row.getByRole('button', { name: 'Delete' }).click()

  // Confirm in popconfirm
  await page.locator('.ant-popconfirm').waitFor({ state: 'visible' })
  await page.locator('.ant-popconfirm').getByRole('button', { name: 'Yes' }).click()

  // Wait for popconfirm to close
  await page.locator('.ant-popconfirm').waitFor({ state: 'hidden' })
}

/**
 * Search and Sort helpers
 */

export async function searchAssistants(page: Page, query: string) {
  const searchInput = page.getByPlaceholder('Search assistants').or(page.locator('input[placeholder="Search assistants"]'))
  await searchInput.fill(query)
}

export async function clearSearch(page: Page) {
  const clearButton = page.getByRole('button', { name: /clear.*search/i })
    .or(page.locator('input[placeholder="Search assistants"]').locator('..').locator('.ant-input-clear-icon'))
  await clearButton.click()
}

export async function sortAssistantsBy(page: Page, sortType: 'Activity' | 'Name' | 'Created') {
  // Click sort button using aria-label
  await page.getByRole('button', { name: /sort assistants/i }).click()

  // Wait for the sort dropdown to appear (identify it by looking for one that contains "Activity")
  const sortDropdown = page.getByRole('menu').or(page.locator('.ant-dropdown-menu:has-text("Activity")'))
  await sortDropdown.waitFor({ state: 'visible', timeout: 10000 })

  // Click the sort option within the specific dropdown
  await sortDropdown.getByText(sortType, { exact: true }).click()

  // Wait for dropdown to close
  await page.waitForTimeout(500)
}

/**
 * Pagination helpers
 */

export async function goToPage(page: Page, pageNumber: number) {
  await page.getByRole('button', { name: `${pageNumber}` }).or(page.locator(`.ant-pagination-item-${pageNumber}`)).click()
  await page.waitForLoadState('networkidle')
}

export async function changePageSize(page: Page, size: number) {
  // Click the page size selector (find by aria-label or any current value)
  await page.locator('.ant-select-selector').filter({ hasText: '/ page' }).click()
  await page.locator('.ant-select-dropdown').waitFor({ state: 'visible' })
  // Match the actual dropdown option text format: "20 / page"
  await page.locator('.ant-select-dropdown').getByText(`${size} / page`, { exact: true }).click()
  await page.waitForLoadState('networkidle')
}

/**
 * Assertion helpers
 */

export async function assertAssistantExists(page: Page, assistantName: string, shouldExist = true) {
  const card = page.locator(`[data-test-assistant-name="${assistantName}"]`)
  if (shouldExist) {
    await expect(card).toBeVisible()
  } else {
    await expect(card).not.toBeVisible()
  }
}

export async function assertTemplateAssistantExists(page: Page, assistantName: string, shouldExist = true) {
  // Find the template assistant row using data-test-assistant-id and text content
  const row = page.locator(`[data-test-assistant-id^="template-assistant-"]:has-text("${assistantName}")`)
  if (shouldExist) {
    await expect(row.first()).toBeVisible()
  } else {
    await expect(row).not.toBeVisible()
  }
}

export async function assertAssistantHasTag(page: Page, assistantName: string, tagText: string) {
  const card = await getAssistantCard(page, assistantName)
  const tag = card.getByText(tagText, { exact: true })
  await expect(tag).toBeVisible()
}

export async function assertEmptyState(page: Page, message: string) {
  await expect(page.getByText(message, { exact: true })).toBeVisible()
}

export async function assertSuccessMessage(page: Page, message: string) {
  // Use .last() to get the most recent message, in case multiple are visible
  await expect(page.getByText(message).or(page.locator(`.ant-message-success:has-text("${message}")`)).last()).toBeVisible({ timeout: 5000 })
}
