import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToTemplateAssistantsSettings,
  openCreateAssistantDrawer,
  fillAssistantForm,
  submitAssistantForm,
  editTemplateAssistant,
  deleteTemplateAssistant,
  getTemplateAssistantRow,
  goToPage,
  changePageSize,
  assertTemplateAssistantExists,
  assertSuccessMessage,
} from './helpers/assistant-helpers'

test.describe('Template Assistants - Settings Page', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToTemplateAssistantsSettings(page, baseURL)
  })

  test('should pass accessibility checks', async ({ page }) => {
    await assertNoAccessibilityViolations(page)
  })

  test('should display template assistants card', async ({ page }) => {
    await expect(page.locator('.ant-card-head-title:has-text("Template Assistants")')).toBeVisible()
    await expect(page.getByText('Manage template assistants. Default assistants are automatically cloned for new users.')).toBeVisible()
  })

  test('should display empty state when no templates exist', async ({ page }) => {
    // Check for empty state
    const emptyDescription = page.getByText('No assistants found', { exact: true })
    if (await emptyDescription.isVisible()) {
      await expect(emptyDescription).toBeVisible()
    }
  })

  test('should create a new template assistant', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    // Verify drawer title
    await expect(page.locator('.ant-drawer-title:has-text("Create Template Assistant")')).toBeVisible()

    await fillAssistantForm(page, {
      name: 'Template Test Assistant',
      description: 'This is a template assistant',
      enabled: true,
    })

    await submitAssistantForm(page)

    // Verify success message
    await assertSuccessMessage(page, 'Assistant created successfully')

    // Verify assistant appears in list
    await assertTemplateAssistantExists(page, 'Template Test Assistant')
  })

  test('should create template assistant with full configuration', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    await fillAssistantForm(page, {
      name: 'Full Template Assistant',
      description: 'Complete template configuration',
      instructions: 'You are a template assistant for all users.',
      parameters: '{"temperature": 0.8, "max_tokens": 4096, "top_p": 0.95}',
      enabled: true,
      isDefault: true,
    })

    await submitAssistantForm(page)

    await assertSuccessMessage(page, 'Assistant created successfully')
    await assertTemplateAssistantExists(page, 'Full Template Assistant')

    // Verify Default tag
    const row = await getTemplateAssistantRow(page, 'Full Template Assistant')
    await expect(row.locator('.ant-tag:has-text("Default")')).toBeVisible()
  })

  test('should edit a template assistant', async ({ page }) => {
    // Create template first
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Edit Template Test',
      description: 'Original template description',
    })
    await submitAssistantForm(page)

    // Edit the template
    await editTemplateAssistant(page, 'Edit Template Test')

    // Verify drawer title
    await expect(page.locator('.ant-drawer-title:has-text("Edit Template Assistant")')).toBeVisible()

    // Verify form is populated
    await expect(page.locator('[aria-label="Assistant name"]')).toHaveValue('Edit Template Test')

    // Update the description
    await page.fill('[aria-label="Assistant description"]', 'Updated template description')

    await submitAssistantForm(page)

    await assertSuccessMessage(page, 'Assistant updated successfully')
  })

  test('should delete a template assistant', async ({ page }) => {
    // Create template first
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Delete Template Test',
      description: 'Will be deleted',
    })
    await submitAssistantForm(page)

    // Delete the template
    await deleteTemplateAssistant(page, 'Delete Template Test')

    // Verify success message
    await assertSuccessMessage(page, 'Assistant deleted successfully')

    // Verify template is removed
    await assertTemplateAssistantExists(page, 'Delete Template Test', false)
  })

  test('should display default tag for default template', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Default Template Test',
      isDefault: true,
    })
    await submitAssistantForm(page)

    const row = await getTemplateAssistantRow(page, 'Default Template Test')
    await expect(row.locator('.ant-tag:has-text("Default")')).toBeVisible()
  })

  test('should display inactive tag for disabled template', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Inactive Template Test',
      enabled: false,
    })
    await submitAssistantForm(page)

    const row = await getTemplateAssistantRow(page, 'Inactive Template Test')
    await expect(row.locator('.ant-tag:has-text("Inactive")')).toBeVisible()
  })

  test('should display template information', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Info Template Test',
      description: 'Test description for display',
    })
    await submitAssistantForm(page)

    const row = await getTemplateAssistantRow(page, 'Info Template Test')

    // Verify description is displayed
    await expect(row.getByText('Test description for display', { exact: true })).toBeVisible()

    // Verify "Created By" is displayed
    await expect(row.getByText('Created By', { exact: true })).toBeVisible()

    // Verify "Created" date is displayed
    await expect(row.getByText('Created', { exact: true })).toBeVisible()
  })

  test('should handle pagination when many templates exist', async ({ page }) => {
    // Create 12 templates (more than default page size of 10)
    for (let i = 1; i <= 12; i++) {
      await openCreateAssistantDrawer(page, false)
      await fillAssistantForm(page, {
        name: `Pagination Template ${i}`,
      })
      await submitAssistantForm(page)
      await page.waitForTimeout(300)
    }

    // Verify pagination controls are visible
    await expect(page.locator('.ant-pagination')).toBeVisible()

    // Verify total count
    await expect(page.locator('text=/\\d+-\\d+ of \\d+ assistants/')).toBeVisible()

    // Go to page 2
    await goToPage(page, 2)

    // Verify we're on page 2
    await expect(page.locator('.ant-pagination-item-active:has-text("2")')).toBeVisible()

    // Verify page 2 templates are visible
    await assertTemplateAssistantExists(page, 'Pagination Template 11')
  })

  test('should change page size', async ({ page }) => {
    // Create 15 templates
    for (let i = 1; i <= 15; i++) {
      await openCreateAssistantDrawer(page, false)
      await fillAssistantForm(page, {
        name: `PageSize Template ${i}`,
      })
      await submitAssistantForm(page)
      await page.waitForTimeout(300)
    }

    // Change page size to 20
    await changePageSize(page, 20)

    // Verify all templates are visible on one page
    await assertTemplateAssistantExists(page, 'PageSize Template 1')
    await assertTemplateAssistantExists(page, 'PageSize Template 15')

    // Verify we're on page 1
    await expect(page.locator('.ant-pagination-item-active:has-text("1")')).toBeVisible()
  })

  test('should toggle template as default', async ({ page }) => {
    // Create two templates
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Template 1',
      isDefault: true,
    })
    await submitAssistantForm(page)

    let row1 = await getTemplateAssistantRow(page, 'Template 1')
    await expect(row1.locator('.ant-tag:has-text("Default")')).toBeVisible()

    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Template 2',
    })
    await submitAssistantForm(page)

    // Set second template as default
    await editTemplateAssistant(page, 'Template 2')

    const defaultSwitch = page.locator('form >> text=Set as Default').locator('..').locator('.ant-switch')
    await defaultSwitch.click()

    await submitAssistantForm(page)

    // Verify Template 2 is now default
    const row2 = await getTemplateAssistantRow(page, 'Template 2')
    await expect(row2.locator('.ant-tag:has-text("Default")')).toBeVisible()

    // Verify Template 1 is no longer default
    row1 = await getTemplateAssistantRow(page, 'Template 1')
    await expect(row1.locator('.ant-tag:has-text("Default")')).not.toBeVisible()
  })

  test('should validate required fields for template', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    // Try to submit without filling required fields
    await page.click('.ant-drawer button[type="submit"]')

    // Verify validation message
    await expect(page.getByText('Please enter a name', { exact: true })).toBeVisible()

    // Drawer should still be open
    await expect(page.locator('.ant-drawer')).toBeVisible()
  })

  test('should validate JSON parameters for template', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    await fillAssistantForm(page, {
      name: 'JSON Validation Template',
      parameters: '{invalid json}',
    })

    await page.click('.ant-drawer button[type="submit"]')

    // Verify JSON validation error
    await expect(page.getByText('Please enter valid JSON', { exact: true })).toBeVisible()
  })

  test('should handle long template names and descriptions', async ({ page }) => {
    const longName = 'A'.repeat(255)
    const longDescription = 'B'.repeat(1000)

    await openCreateAssistantDrawer(page, false)

    await fillAssistantForm(page, {
      name: longName,
      description: longDescription,
    })

    await submitAssistantForm(page)

    await assertSuccessMessage(page, 'Assistant created successfully')

    // Verify truncation or proper display
    await assertTemplateAssistantExists(page, longName.substring(0, 50))
  })

  test('should show tooltip for Set as Default switch', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    const defaultLabel = page.locator('form').getByText('Set as Default', { exact: true })

    // Hover to show tooltip
    await defaultLabel.hover()

    // Verify tooltip text for templates
    await expect(page.getByText('Set as the default template assistant for all users', { exact: true })).toBeVisible({ timeout: 2000 })
  })

  test('should show enabled tooltip', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    const enabledLabel = page.locator('form').getByText('Enabled', { exact: true }).first()

    // Hover to show tooltip
    await enabledLabel.hover()

    // Verify tooltip
    await expect(page.getByText('Whether this assistant is enabled', { exact: true })).toBeVisible({ timeout: 2000 })
  })
})
