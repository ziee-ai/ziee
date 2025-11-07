import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToUserAssistantsPage,
  openCreateAssistantDrawer,
  fillAssistantForm,
  submitAssistantForm,
  cancelAssistantForm,
  editAssistantFromCard,
  deleteAssistantFromCard,
  clickAssistantCard,
  searchAssistants,
  clearSearch,
  sortAssistantsBy,
  assertAssistantExists,
  assertAssistantHasTag,
  assertEmptyState,
  assertSuccessMessage,
} from './helpers/assistant-helpers'

test.describe('User Assistants - User Page', () => {
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
    await assertEmptyState(page, 'Create your first assistant to get started')
    // The primary button in the empty state (not the icon button in the header)
    await expect(page.getByRole('button', { name: 'plus Create Assistant' })).toBeVisible()
  })

  test('should create a new assistant with basic info', async ({ page }) => {
    await openCreateAssistantDrawer(page, true)

    // Verify drawer title
    await expect(page.locator('.ant-drawer-title:has-text("Create Assistant")')).toBeVisible()

    await fillAssistantForm(page, {
      name: 'Test Assistant',
      description: 'This is a test assistant',
      enabled: true,
    })

    await submitAssistantForm(page)

    // Verify success message
    await assertSuccessMessage(page, 'Assistant created successfully')

    // Verify assistant appears in list
    await assertAssistantExists(page, 'Test Assistant')
  })

  test('should create assistant with full configuration', async ({ page }) => {
    await openCreateAssistantDrawer(page, true)

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
    await assertAssistantExists(page, 'Full Config Assistant')
    await assertAssistantHasTag(page, 'Full Config Assistant', 'Default')
  })

  test('should validate required fields', async ({ page }) => {
    await openCreateAssistantDrawer(page, true)

    // Try to submit without filling required fields
    await page.click('.ant-drawer button[type="submit"]')

    // Verify validation message
    await expect(page.getByText('Please enter a name', { exact: true })).toBeVisible()

    // Drawer should still be open
    await expect(page.locator('.ant-drawer')).toBeVisible()
  })

  test('should validate JSON parameters', async ({ page }) => {
    await openCreateAssistantDrawer(page, true)

    await fillAssistantForm(page, {
      name: 'JSON Test Assistant',
      parameters: 'invalid json',
    })

    await page.click('.ant-drawer button[type="submit"]')

    // Verify JSON validation error
    await expect(page.getByText('Please enter valid JSON', { exact: true })).toBeVisible()
  })

  test('should prettify JSON parameters on blur', async ({ page }) => {
    await openCreateAssistantDrawer(page, true)

    const parametersField = page.locator('[aria-label="Model parameters in JSON format"]')

    // Fill with compact JSON
    await parametersField.fill('{"temperature":0.7,"max_tokens":2048}')

    // Blur the field
    await parametersField.blur()

    // Wait a bit for prettification
    await page.waitForTimeout(300)

    // Verify it's been prettified (has newlines and indentation)
    const value = await parametersField.inputValue()
    expect(value).toContain('\n')
    expect(value).toContain('  ')
  })

  test('should edit an existing assistant', async ({ page }) => {
    // Create assistant first
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, {
      name: 'Edit Test Assistant',
      description: 'Original description',
    })
    await submitAssistantForm(page)

    // Edit the assistant
    await editAssistantFromCard(page, 'Edit Test Assistant')

    // Verify drawer title
    await expect(page.locator('.ant-drawer-title:has-text("Edit Assistant")')).toBeVisible()

    // Verify form is populated
    await expect(page.locator('[aria-label="Assistant name"]')).toHaveValue('Edit Test Assistant')
    await expect(page.locator('[aria-label="Assistant description"]')).toHaveValue('Original description')

    // Update the description
    await page.fill('[aria-label="Assistant description"]', 'Updated description')

    await submitAssistantForm(page)

    await assertSuccessMessage(page, 'Assistant updated successfully')
  })

  test('should delete an assistant', async ({ page }) => {
    // Create assistant first
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, {
      name: 'Delete Test Assistant',
      description: 'Will be deleted',
    })
    await submitAssistantForm(page)

    // Delete the assistant
    await deleteAssistantFromCard(page, 'Delete Test Assistant')

    // Verify success message
    await assertSuccessMessage(page, 'Assistant deleted successfully')

    // Verify assistant is removed
    await assertAssistantExists(page, 'Delete Test Assistant', false)
  })

  test('should open edit drawer when clicking on card', async ({ page }) => {
    // Create assistant first
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, {
      name: 'Click Test Assistant',
    })
    await submitAssistantForm(page)

    // Click on the card
    await clickAssistantCard(page, 'Click Test Assistant')

    // Verify edit drawer opens
    await expect(page.locator('.ant-drawer-title:has-text("Edit Assistant")')).toBeVisible()
  })

  test('should cancel assistant creation', async ({ page }) => {
    await openCreateAssistantDrawer(page, true)

    await fillAssistantForm(page, {
      name: 'Cancelled Assistant',
      description: 'This should not be created',
    })

    await cancelAssistantForm(page)

    // Verify assistant was not created
    await assertAssistantExists(page, 'Cancelled Assistant', false)
  })

  test('should search assistants by name', async ({ page }) => {
    // Create multiple assistants
    for (const name of ['Alpha Assistant', 'Beta Assistant', 'Gamma Assistant']) {
      await openCreateAssistantDrawer(page, true)
      await fillAssistantForm(page, { name })
      await submitAssistantForm(page)
      await assertSuccessMessage(page, 'Assistant created successfully')
    }

    // Search for specific assistant
    await searchAssistants(page, 'Beta')

    // Verify only matching assistant is visible
    await assertAssistantExists(page, 'Beta Assistant', true)
    await assertAssistantExists(page, 'Alpha Assistant', false)
    await assertAssistantExists(page, 'Gamma Assistant', false)
  })

  test('should search assistants by description', async ({ page }) => {
    // Create assistants with different descriptions
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, {
      name: 'Search Test 1',
      description: 'This is a coding assistant',
    })
    await submitAssistantForm(page)

    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, {
      name: 'Search Test 2',
      description: 'This is a writing assistant',
    })
    await submitAssistantForm(page)

    // Search by description
    await searchAssistants(page, 'coding')

    await assertAssistantExists(page, 'Search Test 1', true)
    await assertAssistantExists(page, 'Search Test 2', false)
  })

  test('should clear search filter', async ({ page }) => {
    // Create assistants
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, { name: 'Clear Search Test 1' })
    await submitAssistantForm(page)

    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, { name: 'Clear Search Test 2' })
    await submitAssistantForm(page)

    // Apply search
    await searchAssistants(page, 'Test 1')
    await assertAssistantExists(page, 'Clear Search Test 1', true)
    await assertAssistantExists(page, 'Clear Search Test 2', false)

    // Clear search
    await clearSearch(page)

    // Verify all assistants are visible
    await assertAssistantExists(page, 'Clear Search Test 1', true)
    await assertAssistantExists(page, 'Clear Search Test 2', true)
  })

  test('should display empty state when search has no results', async ({ page }) => {
    // Create an assistant
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, { name: 'Empty Search Test' })
    await submitAssistantForm(page)

    // Search for non-existent assistant
    await searchAssistants(page, 'nonexistent')

    await assertEmptyState(page, 'No assistants found')
    await assertEmptyState(page, 'Try adjusting your search criteria')
  })

  test('should sort assistants by name', async ({ page }) => {
    // Create assistants with different names
    for (const name of ['Zebra Assistant', 'Alpha Assistant', 'Middle Assistant']) {
      await openCreateAssistantDrawer(page, true)
      await fillAssistantForm(page, { name })
      await submitAssistantForm(page)
      await assertSuccessMessage(page, 'Assistant created successfully')
    }

    // Sort by name
    await sortAssistantsBy(page, 'Name')

    // Get all assistant card names (first strong text in each card)
    const cards = await page.locator('.ant-card').all()
    const names = await Promise.all(cards.map(card => card.locator('.ant-typography strong').first().textContent()))

    // Verify they are in alphabetical order
    expect(names[0]).toContain('Alpha')
    expect(names[1]).toContain('Middle')
    expect(names[2]).toContain('Zebra')
  })

  test('should sort assistants by activity (most recently updated)', async ({ page }) => {
    // Create three assistants
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, { name: 'First Assistant' })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForTimeout(1000)

    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, { name: 'Second Assistant' })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForTimeout(1000)

    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, { name: 'Third Assistant' })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForTimeout(1000)

    // Edit the first assistant to make it most recently updated
    await editAssistantFromCard(page, 'First Assistant')
    await page.fill('[aria-label="Assistant description"]', 'Updated to be most recent')
    await submitAssistantForm(page)
    await page.waitForTimeout(500)

    // Sort by activity (default, but click to ensure)
    await sortAssistantsBy(page, 'Activity')

    // Get all assistant card names (first strong text in each card)
    const cards = await page.locator('.ant-card').all()
    const names = await Promise.all(cards.map(card => card.locator('.ant-typography strong').first().textContent()))

    // Verify First Assistant is now at the top (most recently updated)
    expect(names[0]).toContain('First Assistant')
  })

  test('should sort assistants by created date', async ({ page }) => {
    // Create assistants in specific order
    for (const name of ['Oldest Assistant', 'Middle Assistant', 'Newest Assistant']) {
      await openCreateAssistantDrawer(page, true)
      await fillAssistantForm(page, { name })
      await submitAssistantForm(page)
      await assertSuccessMessage(page, 'Assistant created successfully')
      await page.waitForTimeout(1000) // Ensure different creation times
    }

    // Sort by created date
    await sortAssistantsBy(page, 'Created')

    // Get all assistant card names (first strong text in each card)
    const cards = await page.locator('.ant-card').all()
    const names = await Promise.all(cards.map(card => card.locator('.ant-typography strong').first().textContent()))

    // Verify they are in reverse chronological order (newest first)
    expect(names[0]).toContain('Newest')
    expect(names[1]).toContain('Middle')
    expect(names[2]).toContain('Oldest')
  })

  test('should toggle assistant as default', async ({ page }) => {
    // Create two assistants
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, {
      name: 'Assistant 1',
      isDefault: true,
    })
    await submitAssistantForm(page)

    // Wait for success message
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForLoadState('networkidle')

    await assertAssistantHasTag(page, 'Assistant 1', 'Default')

    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, {
      name: 'Assistant 2',
    })
    await submitAssistantForm(page)

    // Wait for success message
    await assertSuccessMessage(page, 'Assistant created successfully')
    await page.waitForLoadState('networkidle')

    // Set second assistant as default
    await editAssistantFromCard(page, 'Assistant 2')

    const defaultSwitch = page.locator('form >> text=Set as Default').locator('..').locator('.ant-switch')
    await defaultSwitch.waitFor({ state: 'visible', timeout: 10000 })
    await defaultSwitch.click()

    await submitAssistantForm(page)

    // Wait for success message
    await assertSuccessMessage(page, 'Assistant updated successfully')
    await page.waitForLoadState('networkidle')

    // Verify Assistant 2 is now default
    await assertAssistantHasTag(page, 'Assistant 2', 'Default')

    // Verify Assistant 1 is no longer default (should not have Default tag visible)
    const assistant1Card = page.locator('.ant-card:has-text("Assistant 1")')
    const defaultTag = assistant1Card.locator('.ant-tag:has-text("Default")')
    await expect(defaultTag).not.toBeVisible()
  })

  test('should toggle assistant enabled status', async ({ page }) => {
    // Create enabled assistant
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, {
      name: 'Enabled Test Assistant',
      enabled: true,
    })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')

    // Disable it
    await editAssistantFromCard(page, 'Enabled Test Assistant')

    const enabledSwitch = page.locator('form >> text=Enabled').locator('..').locator('.ant-switch')
    await enabledSwitch.waitFor({ state: 'visible', timeout: 10000 })
    await enabledSwitch.click()

    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant updated successfully')

    // Verify Inactive tag appears
    await assertAssistantHasTag(page, 'Enabled Test Assistant', 'Inactive')
  })

  test('should display creation date on cards', async ({ page }) => {
    await openCreateAssistantDrawer(page, true)
    await fillAssistantForm(page, { name: 'Date Test Assistant' })
    await submitAssistantForm(page)

    const card = page.locator('.ant-card:has-text("Date Test Assistant")')

    // Verify "Updated" text is visible (with relative time)
    await expect(card.locator('text=/Updated/')).toBeVisible()
  })
})
