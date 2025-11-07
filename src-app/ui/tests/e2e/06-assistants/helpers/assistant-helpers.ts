import { Page, expect } from '@playwright/test'

/**
 * Navigation helpers for assistants pages
 */

export async function goToUserAssistantsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/assistants`)
  await page.waitForLoadState('networkidle')
  // Wait for the page title (h4 heading in title bar) specifically, not the empty state heading (h3)
  await page.locator('h4:has-text("Assistants")').first().waitFor({ timeout: 10000 })
}

export async function goToTemplateAssistantsSettings(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/assistants`)
  await page.waitForLoadState('networkidle')
  await page.locator('.ant-card-head-title:has-text("Template Assistants")').waitFor({ timeout: 10000 })
}

/**
 * Assistant Form Drawer helpers
 */

export async function openCreateAssistantDrawer(page: Page, _isUserPage = true) {
  // Both pages now use the same aria-label for the create button
  await page.click('button[aria-label="Create assistant"]')
  await page.waitForSelector('.ant-drawer', { state: 'visible' })
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
  await page.waitForSelector('[aria-label="Assistant name"]', { state: 'visible' })

  // Fill name
  await page.fill('[aria-label="Assistant name"]', data.name)

  // Fill description if provided
  if (data.description !== undefined) {
    await page.fill('[aria-label="Assistant description"]', data.description)
  }

  // Fill instructions if provided
  if (data.instructions !== undefined) {
    await page.fill('[aria-label="Assistant instructions"]', data.instructions)
  }

  // Fill parameters if provided
  if (data.parameters !== undefined) {
    await page.fill('[aria-label="Model parameters in JSON format"]', data.parameters)
  }

  // Set enabled toggle
  if (data.enabled !== undefined) {
    // Find the Form.Item containing "Enabled" label, then find the switch within it
    const enabledFormItem = page.locator('.ant-form-item').filter({ hasText: /^Enabled/ })
    const enabledSwitch = enabledFormItem.locator('.ant-switch')
    await enabledSwitch.waitFor({ state: 'visible', timeout: 5000 })
    const isEnabled = await enabledSwitch.getAttribute('aria-checked')
    if ((isEnabled === 'true') !== data.enabled) {
      await enabledSwitch.click()
    }
  }

  // Set default toggle
  if (data.isDefault !== undefined) {
    // Find the Form.Item containing "Set as Default" label, then find the switch within it
    const defaultFormItem = page.locator('.ant-form-item').filter({ hasText: /^Set as Default/ })
    const defaultSwitch = defaultFormItem.locator('.ant-switch')
    await defaultSwitch.waitFor({ state: 'visible', timeout: 5000 })
    const isDefault = await defaultSwitch.getAttribute('aria-checked')
    if ((isDefault === 'true') !== data.isDefault) {
      await defaultSwitch.click()
    }
  }
}

export async function submitAssistantForm(page: Page) {
  await page.click('.ant-drawer button[type="submit"]')
  // Don't wait for drawer here - let the test verify success message first
  // The drawer will close automatically after successful submission
}

export async function cancelAssistantForm(page: Page) {
  await page.locator('.ant-drawer').getByRole('button', { name: 'Cancel' }).click()
  await page.waitForSelector('.ant-drawer', { state: 'hidden', timeout: 10000 })
}

/**
 * Assistant Card helpers (User Assistants Page)
 */

export async function getAssistantCard(page: Page, assistantName: string) {
  return page.locator(`.ant-card:has-text("${assistantName}")`)
}

export async function editAssistantFromCard(page: Page, assistantName: string) {
  const card = await getAssistantCard(page, assistantName)

  // Click the menu button
  await card.locator('button:has(svg)').last().click()

  // Wait for dropdown menu
  await page.waitForSelector('.ant-dropdown-menu', { state: 'visible' })

  // Click Edit
  await page.locator('.ant-dropdown-menu').getByText('Edit', { exact: true }).click()

  // Wait for drawer to open
  await page.waitForSelector('.ant-drawer', { state: 'visible' })

  // Wait for form content to be loaded (same as fillAssistantForm does)
  await page.waitForSelector('[aria-label="Assistant name"]', { state: 'visible', timeout: 10000 })
}

export async function deleteAssistantFromCard(page: Page, assistantName: string) {
  const card = await getAssistantCard(page, assistantName)

  // Click the menu button
  await card.locator('button:has(svg)').last().click()

  // Wait for dropdown menu
  await page.waitForSelector('.ant-dropdown-menu', { state: 'visible' })

  // Click Delete
  await page.locator('.ant-dropdown-menu').getByText('Delete', { exact: true }).click()

  // Confirm deletion in modal
  await page.waitForSelector('.ant-modal', { state: 'visible' })
  await page.locator('.ant-modal').getByRole('button', { name: 'Delete' }).click()

  // Wait for modal to close
  await page.waitForSelector('.ant-modal', { state: 'hidden' })
}

export async function clickAssistantCard(page: Page, assistantName: string) {
  const card = await getAssistantCard(page, assistantName)
  await card.click()
  await page.waitForSelector('.ant-drawer', { state: 'visible' })
}

/**
 * Template Assistant List helpers (Settings Page)
 */

export async function getTemplateAssistantRow(page: Page, assistantName: string) {
  // Find the assistant name text, then navigate up to the parent container that has the actions AND descriptions
  return page.locator(`.ant-card-body`).locator(`text=${assistantName}`).locator('..').locator('..').locator('..').locator('..')
}

export async function editTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  await row.getByRole('button', { name: 'Edit' }).click()
  await page.waitForSelector('.ant-drawer', { state: 'visible' })
}

export async function deleteTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  await row.getByRole('button', { name: 'Delete' }).click()

  // Confirm in popconfirm
  await page.waitForSelector('.ant-popconfirm', { state: 'visible' })
  await page.locator('.ant-popconfirm').getByRole('button', { name: 'Yes' }).click()

  // Wait for popconfirm to close
  await page.waitForSelector('.ant-popconfirm', { state: 'hidden' })
}

/**
 * Search and Sort helpers
 */

export async function searchAssistants(page: Page, query: string) {
  const searchInput = page.locator('input[placeholder="Search assistants"]')
  await searchInput.fill(query)
}

export async function clearSearch(page: Page) {
  const clearButton = page.locator('input[placeholder="Search assistants"]').locator('..').locator('.ant-input-clear-icon')
  await clearButton.click()
}

export async function sortAssistantsBy(page: Page, sortType: 'Activity' | 'Name' | 'Created') {
  // Click sort button using aria-label
  await page.click('button[aria-label="Sort assistants"]')

  // Wait for the sort dropdown to appear (identify it by looking for one that contains "Activity")
  const sortDropdown = page.locator('.ant-dropdown-menu:has-text("Activity")')
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
  await page.click(`.ant-pagination-item-${pageNumber}`)
  await page.waitForLoadState('networkidle')
}

export async function changePageSize(page: Page, size: number) {
  // Click the page size selector (find by aria-label or any current value)
  await page.locator('.ant-select-selector').filter({ hasText: '/ page' }).click()
  await page.waitForSelector('.ant-select-dropdown', { state: 'visible' })
  // Match the actual dropdown option text format: "20 / page"
  await page.locator('.ant-select-dropdown').getByText(`${size} / page`, { exact: true }).click()
  await page.waitForLoadState('networkidle')
}

/**
 * Assertion helpers
 */

export async function assertAssistantExists(page: Page, assistantName: string, shouldExist = true) {
  const card = page.locator(`.ant-card:has-text("${assistantName}")`)
  if (shouldExist) {
    await expect(card).toBeVisible()
  } else {
    await expect(card).not.toBeVisible()
  }
}

export async function assertTemplateAssistantExists(page: Page, assistantName: string, shouldExist = true) {
  const row = page.locator(`.ant-card-body >> text=${assistantName}`)
  if (shouldExist) {
    await expect(row).toBeVisible()
  } else {
    await expect(row).not.toBeVisible()
  }
}

export async function assertAssistantHasTag(page: Page, assistantName: string, tagText: string) {
  const card = await getAssistantCard(page, assistantName)
  const tag = card.locator(`.ant-tag:has-text("${tagText}")`)
  await expect(tag).toBeVisible()
}

export async function assertEmptyState(page: Page, message: string) {
  await expect(page.getByText(message, { exact: true })).toBeVisible()
}

export async function assertSuccessMessage(page: Page, message: string) {
  // Use .last() to get the most recent message, in case multiple are visible
  await expect(page.locator(`.ant-message-success:has-text("${message}")`).last()).toBeVisible({ timeout: 5000 })
}
