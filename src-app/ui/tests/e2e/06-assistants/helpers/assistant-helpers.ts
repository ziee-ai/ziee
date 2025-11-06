import { Page, expect } from '@playwright/test'

/**
 * Navigation helpers for assistants pages
 */

export async function goToUserAssistantsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/assistants`)
  await page.waitForLoadState('networkidle')
  await page.waitForSelector('text=Assistants', { timeout: 10000 })
}

export async function goToTemplateAssistantsSettings(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/assistants`)
  await page.waitForLoadState('networkidle')
  await page.waitForSelector('text=Template Assistants', { timeout: 10000 })
}

/**
 * Assistant Form Drawer helpers
 */

export async function openCreateAssistantDrawer(page: Page, isUserPage = true) {
  if (isUserPage) {
    // On user assistants page, click the + button in header
    await page.locator('button:has-text("")[has(svg)]').last().click()
  } else {
    // On template settings page, click the + button in card header
    await page.locator('.ant-card-head button:has(svg)').click()
  }
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
    const enabledSwitch = page.locator('form >> text=Enabled').locator('..').locator('.ant-switch')
    const isEnabled = await enabledSwitch.getAttribute('aria-checked')
    if ((isEnabled === 'true') !== data.enabled) {
      await enabledSwitch.click()
    }
  }

  // Set default toggle
  if (data.isDefault !== undefined) {
    const defaultSwitch = page.locator('form >> text=Set as Default').locator('..').locator('.ant-switch')
    const isDefault = await defaultSwitch.getAttribute('aria-checked')
    if ((isDefault === 'true') !== data.isDefault) {
      await defaultSwitch.click()
    }
  }
}

export async function submitAssistantForm(page: Page) {
  await page.click('.ant-drawer button[type="submit"]')
  await page.waitForSelector('.ant-drawer', { state: 'hidden', timeout: 10000 })
}

export async function cancelAssistantForm(page: Page) {
  await page.click('.ant-drawer button:has-text("Cancel")')
  await page.waitForSelector('.ant-drawer', { state: 'hidden', timeout: 5000 })
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
  await page.click('.ant-dropdown-menu >> text=Edit')

  // Wait for drawer to open
  await page.waitForSelector('.ant-drawer', { state: 'visible' })
}

export async function deleteAssistantFromCard(page: Page, assistantName: string) {
  const card = await getAssistantCard(page, assistantName)

  // Click the menu button
  await card.locator('button:has(svg)').last().click()

  // Wait for dropdown menu
  await page.waitForSelector('.ant-dropdown-menu', { state: 'visible' })

  // Click Delete
  await page.click('.ant-dropdown-menu >> text=Delete')

  // Confirm deletion in modal
  await page.waitForSelector('.ant-modal', { state: 'visible' })
  await page.click('.ant-modal button:has-text("Delete")')

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
  return page.locator(`.ant-card-body >> text=${assistantName}`).locator('..')
}

export async function editTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  await row.locator('button:has-text("Edit")').click()
  await page.waitForSelector('.ant-drawer', { state: 'visible' })
}

export async function deleteTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  await row.locator('button:has-text("Delete")').click()

  // Confirm in popconfirm
  await page.waitForSelector('.ant-popconfirm', { state: 'visible' })
  await page.click('.ant-popconfirm button:has-text("Yes")')

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
  // Click sort button
  await page.click('button:has(svg[class*="PiSortAscending"])')

  // Wait for dropdown
  await page.waitForSelector('.ant-dropdown-menu', { state: 'visible' })

  // Click sort option
  await page.click(`.ant-dropdown-menu >> text=${sortType}`)

  // Wait for dropdown to close
  await page.waitForSelector('.ant-dropdown-menu', { state: 'hidden' })
}

/**
 * Pagination helpers
 */

export async function goToPage(page: Page, pageNumber: number) {
  await page.click(`.ant-pagination-item-${pageNumber}`)
  await page.waitForLoadState('networkidle')
}

export async function changePageSize(page: Page, size: number) {
  await page.click('.ant-select-selector:has-text("10")')
  await page.waitForSelector('.ant-select-dropdown', { state: 'visible' })
  await page.click(`.ant-select-dropdown >> text=${size}`)
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
  await expect(page.locator(`text=${message}`)).toBeVisible()
}

export async function assertSuccessMessage(page: Page, message: string) {
  await expect(page.locator(`.ant-message-success:has-text("${message}")`)).toBeVisible({ timeout: 5000 })
}
