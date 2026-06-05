import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToUserAssistantsPage,
  openCreateAssistantDrawer,
  fillAssistantForm,
  submitAssistantForm,
  cancelAssistantForm,
  editUserAssistant,
  deleteUserAssistant,
  getUserAssistantRow,
  assertUserAssistantExists,
  assertUserAssistantHasTag,
  assertEmptyState,
  assertSuccessMessage,
} from './helpers/assistant-helpers'

// The user's own assistants now live in the settings area
// (/settings/assistants) with the same card-list interface as the admin
// "Assistant Templates" page — no search/sort, inline Edit/Delete per row.
test.describe('User Assistants - Settings Page', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToUserAssistantsPage(page, baseURL)
  })

  test('should pass accessibility checks', async ({ page }) => {
    await assertNoAccessibilityViolations(page)
  })

  test('should display empty state when no assistants exist', async ({ page }) => {
    await assertEmptyState(page, 'No assistants yet')
    // The create affordance is the (+) button in the card header.
    await expect(
      page.getByRole('button', { name: /create assistant/i }),
    ).toBeVisible()
  })

  test('should create a new assistant with basic info', async ({ page }) => {
    await openCreateAssistantDrawer(page)

    // Verify drawer title
    await expect(page.locator('.ant-drawer.ant-drawer-open').getByText('Create Assistant')).toBeVisible()

    await fillAssistantForm(page, {
      name: 'Test Assistant',
      description: 'This is a test assistant',
      enabled: true,
    })

    await submitAssistantForm(page)

    await assertSuccessMessage(page, 'Assistant created successfully')
    await assertUserAssistantExists(page, 'Test Assistant')
  })

  test('should create assistant with full configuration', async ({ page }) => {
    await openCreateAssistantDrawer(page)

    await fillAssistantForm(page, {
      name: 'Full Config Assistant',
      description: 'Assistant with complete configuration',
      instructions: 'You are a helpful assistant that provides detailed explanations.',
      parameters: '{"temperature": 0.7, "max_tokens": 2048}',
      enabled: true,
      isDefault: true,
    })

    await submitAssistantForm(page)

    await assertSuccessMessage(page, 'Assistant created successfully')
    await assertUserAssistantExists(page, 'Full Config Assistant')
    await assertUserAssistantHasTag(page, 'Full Config Assistant', 'Default')
  })

  test('should validate required fields', async ({ page }) => {
    await openCreateAssistantDrawer(page)

    // Try to submit without filling required fields
    await page.locator('.ant-drawer.ant-drawer-open').getByRole('button', { name: 'Create' }).click()

    await expect(page.getByText('Please enter a name', { exact: true })).toBeVisible()

    // Drawer should still be open
    await expect(page.locator('.ant-drawer.ant-drawer-open')).toBeVisible()
  })

  test('should validate JSON parameters', async ({ page }) => {
    await openCreateAssistantDrawer(page)

    await fillAssistantForm(page, {
      name: 'JSON Test Assistant',
      parameters: 'invalid json',
    })

    await page.locator('.ant-drawer.ant-drawer-open').getByRole('button', { name: 'Create' }).click()

    await expect(page.getByText('Please enter valid JSON', { exact: true })).toBeVisible()
  })

  test('should prettify JSON parameters on blur', async ({ page }) => {
    await openCreateAssistantDrawer(page)

    const parametersField = page.getByLabel('Model parameters in JSON format')

    await parametersField.fill('{"temperature":0.7,"max_tokens":2048}')
    await parametersField.blur()
    await page.waitForTimeout(300)

    const value = await parametersField.inputValue()
    expect(value).toContain('\n')
    expect(value).toContain('  ')
  })

  test('should edit an existing assistant', async ({ page }) => {
    // Create assistant first
    await openCreateAssistantDrawer(page)
    await fillAssistantForm(page, {
      name: 'Edit Test Assistant',
      description: 'Original description',
    })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')

    // Edit the assistant from its row
    await editUserAssistant(page, 'Edit Test Assistant')

    await expect(page.locator('.ant-drawer.ant-drawer-open').getByText('Edit Assistant')).toBeVisible()

    // Verify form is populated
    await expect(page.getByLabel('Assistant name')).toHaveValue('Edit Test Assistant')
    await expect(page.getByLabel('Assistant description')).toHaveValue('Original description')

    // Update the description
    await page.getByLabel('Assistant description').fill('Updated description')

    await submitAssistantForm(page)

    await assertSuccessMessage(page, 'Assistant updated successfully')
  })

  test('should delete an assistant', async ({ page }) => {
    // Create assistant first
    await openCreateAssistantDrawer(page)
    await fillAssistantForm(page, {
      name: 'Delete Test Assistant',
      description: 'Will be deleted',
    })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')

    // Delete the assistant from its row
    await deleteUserAssistant(page, 'Delete Test Assistant')

    await assertSuccessMessage(page, 'Assistant deleted successfully')
    await assertUserAssistantExists(page, 'Delete Test Assistant', false)
  })

  test('should cancel assistant creation', async ({ page }) => {
    await openCreateAssistantDrawer(page)

    await fillAssistantForm(page, {
      name: 'Cancelled Assistant',
      description: 'This should not be created',
    })

    await cancelAssistantForm(page)

    await assertUserAssistantExists(page, 'Cancelled Assistant', false)
  })

  test('should toggle assistant as default', async ({ page }) => {
    // Create two assistants
    await openCreateAssistantDrawer(page)
    await fillAssistantForm(page, {
      name: 'Assistant 1',
      isDefault: true,
    })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForLoadState('networkidle')

    await assertUserAssistantHasTag(page, 'Assistant 1', 'Default')

    await openCreateAssistantDrawer(page)
    await fillAssistantForm(page, {
      name: 'Assistant 2',
    })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForLoadState('networkidle')

    // Set second assistant as default
    await editUserAssistant(page, 'Assistant 2')

    const defaultSwitch = page.locator('.ant-form-item:has-text("Set as Default") .ant-switch')
    await defaultSwitch.waitFor({ state: 'visible', timeout: 10000 })
    await defaultSwitch.click()

    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant updated successfully')
    await page.waitForLoadState('networkidle')

    // Reload to force the list to re-fetch — the store emits an event
    // after the PUT but the card binding doesn't always observe it on
    // the same tick.
    await page.reload({ waitUntil: 'networkidle' })

    // Verify Assistant 2 is now default and Assistant 1 is not
    await assertUserAssistantHasTag(page, 'Assistant 2', 'Default')

    const assistant1Row = await getUserAssistantRow(page, 'Assistant 1')
    const defaultTag = assistant1Row.getByText('Default', { exact: true })
    await expect(defaultTag).not.toBeVisible()
  })

  test('should toggle assistant enabled status', async ({ page }) => {
    // Create enabled assistant
    await openCreateAssistantDrawer(page)
    await fillAssistantForm(page, {
      name: 'Enabled Test Assistant',
      enabled: true,
    })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')

    // Disable it
    await editUserAssistant(page, 'Enabled Test Assistant')

    const enabledSwitch = page.locator('.ant-form-item:has-text("Enabled") .ant-switch')
    await enabledSwitch.waitFor({ state: 'visible', timeout: 10000 })
    await enabledSwitch.click()

    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant updated successfully')

    // Verify Inactive tag appears
    await assertUserAssistantHasTag(page, 'Enabled Test Assistant', 'Inactive')
  })

  test('should display creation date on rows', async ({ page }) => {
    await openCreateAssistantDrawer(page)
    await fillAssistantForm(page, { name: 'Date Test Assistant' })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')

    const row = await getUserAssistantRow(page, 'Date Test Assistant')

    // The row shows a "Created" date in the Descriptions block.
    await expect(row.getByText('Created', { exact: true })).toBeVisible()
  })
})
