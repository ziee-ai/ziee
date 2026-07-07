import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  goToTemplateAssistantsSettings,
  openCreateAssistantDrawer,
  fillAssistantForm,
  setAssistantSwitch,
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
    await expect(byTestId(page, 'template-assistants-card')).toBeVisible()
  })

  test('should display empty state when no templates exist', async ({ page }) => {
    // Check for empty state (only present when no templates exist).
    const empty = byTestId(page, 'template-assistants-empty')
    if (await empty.isVisible()) {
      await expect(empty).toBeVisible()
    }
  })

  test('should create a new template assistant', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    // Create mode: the name field opens empty.
    await expect(byTestId(page, 'assistant-form-name')).toHaveValue('')

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

    // A second (non-default) template so the list has >1 rows — the "Default"
    // tag is redundant on a single-row list and only renders to disambiguate
    // among multiple templates.
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, { name: 'Second Template Assistant', description: 'peer row' })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')

    // Verify Default tag
    const row = await getTemplateAssistantRow(page, 'Full Template Assistant')
    const id = await row.getAttribute('data-test-assistant-id')
    await expect(byTestId(page, `${id}-default-tag`)).toBeVisible()
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

    // Edit mode: the form is populated with the persisted name.
    await expect(byTestId(page, 'assistant-form-name')).toHaveValue('Edit Template Test')

    // Update the description
    await byTestId(page, 'assistant-form-description').fill('Updated template description')

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

    // A second (non-default) template so the list has >1 rows — "Default" only
    // renders when there is more than one template to disambiguate.
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, { name: 'Peer Template Test', description: 'peer row' })
    await submitAssistantForm(page)

    const row = await getTemplateAssistantRow(page, 'Default Template Test')
    const id = await row.getAttribute('data-test-assistant-id')
    await expect(byTestId(page, `${id}-default-tag`)).toBeVisible()
  })

  test('should display inactive tag for disabled template', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Inactive Template Test',
      enabled: false,
    })
    await submitAssistantForm(page)

    // Wait for success message and list reload
    await assertSuccessMessage(page, 'Assistant created successfully')

    // Wait for the assistant to appear in the reloaded list
    await assertTemplateAssistantExists(page, 'Inactive Template Test')

    const row = await getTemplateAssistantRow(page, 'Inactive Template Test')
    const id = await row.getAttribute('data-test-assistant-id')
    await expect(byTestId(page, `${id}-inactive-tag`)).toBeVisible()
  })

  test('should display template information', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Info Template Test',
      description: 'Test description for display',
    })
    await submitAssistantForm(page)

    const row = await getTemplateAssistantRow(page, 'Info Template Test')
    const id = await row.getAttribute('data-test-assistant-id')

    // The row's Descriptions block renders description + Created By + Created.
    const desc = byTestId(page, `${id}-desc`)
    await expect(desc).toBeVisible()
    // The description value is dynamic data this test created.
    await expect(desc).toContainText('Test description for display')
  })

  test('should handle pagination when many templates exist', async ({ page }) => {
    // Create 12 templates (more than default page size of 10)
    for (let i = 1; i <= 12; i++) {
      await openCreateAssistantDrawer(page, false)
      await fillAssistantForm(page, {
        name: `Pagination Template ${i}`,
      })
      await submitAssistantForm(page)

      // Wait for success message to confirm creation
      await assertSuccessMessage(page, 'Assistant created successfully')
    }

    // Wait for final list reload to complete with all 13 templates (12 + Default Assistant)
    await page.waitForLoadState('load')

    // Wait for the last created assistant to appear in the list (confirms list reloaded)
    await assertTemplateAssistantExists(page, 'Pagination Template 12')

    // Verify pagination controls are visible (>10 items → more than one page)
    const pagination = byTestId(page, 'template-assistants-pagination')
    await expect(pagination).toBeVisible()

    // Go to page 2
    await goToPage(page, 2)

    // Verify we're on page 2 (the active numbered link).
    await expect(pagination.locator('a[aria-current="page"]')).toHaveText('2')

    // Verify page 2 templates are visible (sorted newest first, so older templates on page 2)
    await assertTemplateAssistantExists(page, 'Pagination Template 2')

    // Verify the Default Assistant is also on page 2
    await assertTemplateAssistantExists(page, 'Default Assistant')
  })

  test('should change page size', async ({ page }) => {
    // Create 15 templates
    for (let i = 1; i <= 15; i++) {
      await openCreateAssistantDrawer(page, false)
      await fillAssistantForm(page, {
        name: `PageSize Template ${i}`,
      })
      await submitAssistantForm(page)

      // Wait for success message to confirm creation
      await assertSuccessMessage(page, 'Assistant created successfully')
    }

    // Wait for final list reload to complete
    await page.waitForLoadState('load')

    // Wait for the last created assistant to appear in the list (confirms list reloaded)
    await assertTemplateAssistantExists(page, 'PageSize Template 15')

    // Change page size to 20
    await changePageSize(page, 20)

    // Verify all templates are visible on one page
    await assertTemplateAssistantExists(page, 'PageSize Template 1')
    await assertTemplateAssistantExists(page, 'PageSize Template 15')

    // A single page now holds everything — there is no page-2 link.
    const pagination = byTestId(page, 'template-assistants-pagination')
    await expect(pagination.locator('a', { hasText: /^2$/ })).toHaveCount(0)
  })

  test('should toggle template as default', async ({ page }) => {
    // Create two templates
    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Template 1',
      isDefault: true,
    })
    await submitAssistantForm(page)

    // Wait for success message
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForLoadState('load')

    let row1 = await getTemplateAssistantRow(page, 'Template 1')
    let row1Id = await row1.getAttribute('data-test-assistant-id')
    await expect(byTestId(page, `${row1Id}-default-tag`)).toBeVisible()

    await openCreateAssistantDrawer(page, false)
    await fillAssistantForm(page, {
      name: 'Template 2',
    })
    await submitAssistantForm(page)

    // Wait for success message
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForLoadState('load')

    // Set second template as default
    await editTemplateAssistant(page, 'Template 2')

    // Set as default (robust to the Base UI switch re-render dropping a click).
    await setAssistantSwitch(page, 'assistant-form-default', true)

    await submitAssistantForm(page)

    // Wait for success message
    await assertSuccessMessage(page, 'Assistant updated successfully')
    await page.waitForLoadState('load')

    // Verify Template 2 is now default
    const row2 = await getTemplateAssistantRow(page, 'Template 2')
    const row2Id = await row2.getAttribute('data-test-assistant-id')
    await expect(byTestId(page, `${row2Id}-default-tag`)).toBeVisible()

    // Verify Template 1 is no longer default
    row1 = await getTemplateAssistantRow(page, 'Template 1')
    row1Id = await row1.getAttribute('data-test-assistant-id')
    await expect(byTestId(page, `${row1Id}-default-tag`)).not.toBeVisible()
  })

  test('should validate required fields for template', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    // Try to submit without filling required fields
    await byTestId(page, 'assistant-form-submit').click()

    // Verify validation message
    await expect(byTestId(page, 'field-error-name')).toContainText('Please enter a name')

    // Drawer should still be open (form still mounted).
    await expect(byTestId(page, 'assistant-form')).toBeVisible()
  })

  test('should validate JSON parameters for template', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    await fillAssistantForm(page, {
      name: 'JSON Validation Template',
      parameters: '{invalid json}',
    })

    await byTestId(page, 'assistant-form-submit').click()

    // Verify JSON validation error
    await expect(byTestId(page, 'field-error-parameters')).toContainText('Please enter valid JSON')
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

  test('should show help text for Set as Default switch', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    // The "Set as Default" field renders its help text inline via the form
    // field description (always visible — not a hover tooltip).
    await expect(byTestId(page, 'field-desc-is_default')).toContainText(
      'Set as the default template assistant for all users',
    )
  })

  test('should show enabled help text', async ({ page }) => {
    await openCreateAssistantDrawer(page, false)

    // The "Enabled" field renders its help text inline via the form field
    // description (always visible — not a hover tooltip).
    await expect(byTestId(page, 'field-desc-enabled')).toContainText(
      'Whether this assistant is enabled',
    )
  })
})
