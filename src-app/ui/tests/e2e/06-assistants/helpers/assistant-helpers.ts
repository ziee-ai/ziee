import { Page, expect } from '@playwright/test'

/**
 * Navigation helpers for assistants pages
 *
 * Uses semantic selectors following best practices
 */

export async function goToUserAssistantsPage(page: Page, baseURL: string) {
  // The user's own assistants now live in settings (was the sidebar
  // full-page grid at /assistants).
  await page.goto(`${baseURL}/settings/assistants`)
  await page.waitForLoadState('networkidle')
  // Wait for the settings page title (h4) then the "My Assistants" card.
  await page.getByRole('heading', { level: 4, name: /assistants/i }).first().waitFor({ timeout: 10000 })
  await page.locator('.ant-card-head-title:has-text("My Assistants")').waitFor({ timeout: 10000 })
}

export async function goToTemplateAssistantsSettings(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/assistant-templates`)
  await page.waitForLoadState('networkidle')
  // Wait for the Assistant Templates heading to be visible first
  await page.getByRole('heading', { name: 'Assistant Templates', level: 4 }).waitFor({ timeout: 10000 })
  // Then wait for the Template Assistants card title specifically
  await page.locator('.ant-card-head-title:has-text("Template Assistants")').waitFor({ timeout: 10000 })
}

/**
 * Assistant Form Drawer helpers
 */

export async function openCreateAssistantDrawer(page: Page, _isUserPage = true) {
  // Both pages now use the same aria-label for the create button
  // Use .first() to handle user page which may have both header button and empty state button
  await page.getByRole('button', { name: /create assistant/i }).first().click()
  // Wait for the drawer to appear - use the outer drawer container which is unique
  await page.locator('.ant-drawer.ant-drawer-open').waitFor({ state: 'visible' })
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
  await page.locator('.ant-drawer.ant-drawer-open').getByRole('button', { name: /submit|create|save|update/i }).click()
  // Don't wait for drawer here - let the test verify success message first
  // The drawer will close automatically after successful submission
}

export async function cancelAssistantForm(page: Page) {
  // Find the Cancel button within any visible drawer
  await page.locator('.ant-drawer.ant-drawer-open').getByRole('button', { name: 'Cancel' }).click()

  // Wait for the drawer to close by checking that no visible drawers remain
  // We check the drawer wrapper class that Ant Design uses when drawer is open
  await page.locator('.ant-drawer.ant-drawer-open').waitFor({
    state: 'hidden',
    timeout: 10000
  })
}

/**
 * User Assistant List helpers (Settings Page)
 *
 * The user's assistants render in the same card-list layout as the admin
 * template page (rows keyed by `data-test-assistant-id="user-assistant-…"`
 * with inline Edit/Delete text buttons), so these mirror the template
 * helpers below.
 */

export async function getUserAssistantRow(page: Page, assistantName: string) {
  return page.locator(`[data-test-assistant-id^="user-assistant-"]:has-text("${assistantName}")`)
}

export async function editUserAssistant(page: Page, assistantName: string) {
  const row = await getUserAssistantRow(page, assistantName)
  await row.getByRole('button', { name: 'Edit' }).click()
  // Wait for the edit drawer to appear + its form to load.
  await page.locator('.ant-drawer.ant-drawer-open').waitFor({ state: 'visible' })
  await page.getByLabel('Name').waitFor({ state: 'visible', timeout: 10000 })
}

export async function deleteUserAssistant(page: Page, assistantName: string) {
  const row = await getUserAssistantRow(page, assistantName)
  await row.getByRole('button', { name: 'Delete' }).click()

  // Confirm in popconfirm. Primary-button class is stable across okText.
  await page.locator('.ant-popconfirm').waitFor({ state: 'visible' })
  await page.locator('.ant-popconfirm .ant-btn-primary').click()
  await page.locator('.ant-popconfirm').waitFor({ state: 'hidden' })
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
  // Wait for the edit drawer to appear
  await page.locator('.ant-drawer.ant-drawer-open').waitFor({ state: 'visible' })
}

export async function deleteTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  await row.getByRole('button', { name: 'Delete' }).click()

  // Confirm in popconfirm. Primary-button class is stable across
  // okText variations ("Yes" → "Delete" per audit I-4).
  await page.locator('.ant-popconfirm').waitFor({ state: 'visible' })
  await page.locator('.ant-popconfirm .ant-btn-primary').click()

  // Wait for popconfirm to close
  await page.locator('.ant-popconfirm').waitFor({ state: 'hidden' })
}

/**
 * Pagination helpers
 */

export async function goToPage(page: Page, pageNumber: number) {
  await page.getByRole('button', { name: `${pageNumber}` }).or(page.locator(`.ant-pagination-item-${pageNumber}`)).click()
  await page.waitForLoadState('networkidle')
}

export async function changePageSize(page: Page, size: number) {
  // Drive the AntD page-size Select via keyboard — option clicks
  // are flaky due to animation/positioning. The combobox is inside
  // the pagination options group.
  const combobox = page
    .locator('.ant-pagination-options')
    .first()
    .getByRole('combobox')
  await combobox.click({ force: true })
  await page.waitForTimeout(300) // open dropdown
  // Page-size options are typically [5, 10, 20, 50] — index of `size`.
  const options = [5, 10, 20, 50, 100]
  const idx = options.indexOf(size)
  if (idx < 0) {
    throw new Error(`Unsupported pageSize ${size}; helper supports ${options.join(', ')}`)
  }
  await combobox.press('Home')
  for (let i = 0; i < idx; i++) {
    await combobox.press('ArrowDown')
  }
  await combobox.press('Enter')
  await page.waitForLoadState('networkidle')
}

/**
 * Assertion helpers
 */

export async function assertUserAssistantExists(page: Page, assistantName: string, shouldExist = true) {
  const row = page.locator(`[data-test-assistant-id^="user-assistant-"]:has-text("${assistantName}")`)
  if (shouldExist) {
    await expect(row.first()).toBeVisible()
  } else {
    await expect(row).not.toBeVisible()
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

export async function assertUserAssistantHasTag(page: Page, assistantName: string, tagText: string) {
  const row = await getUserAssistantRow(page, assistantName)
  const tag = row.getByText(tagText, { exact: true })
  await expect(tag).toBeVisible()
}

export async function assertEmptyState(page: Page, message: string) {
  await expect(page.getByText(message, { exact: true })).toBeVisible()
}

export async function assertSuccessMessage(page: Page, message: string) {
  // Use .last() to get the most recent message, in case multiple are visible
  await expect(page.getByText(message).or(page.locator(`.ant-message-success:has-text("${message}")`)).last()).toBeVisible({ timeout: 5000 })
}
