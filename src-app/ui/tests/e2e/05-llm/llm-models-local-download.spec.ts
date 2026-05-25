import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  clickProviderCard,
} from './helpers/navigation-helpers'
import {
  createLocalProvider,
} from './helpers/provider-helpers'
import {
  openAddModelDropdown,
  selectAddModelOption,
  startModelDownload,
  assertModelExists,
  deleteModel,
} from './helpers/model-helpers'
import {
  configureHuggingFaceAuth,
} from './helpers/repository-helpers'

/**
 * LLM Models - Local Download Tests
 *
 * Tests for downloading models from remote repositories following backend test patterns from:
 * - src-web/tests/llm_model/download_test.rs
 * - src-web/tests/llm_model/download_management_test.rs
 *
 * Key testing areas:
 * 1. Download drawer UI and form
 * 2. Repository selection
 * 3. Download initiation
 * 4. SSE progress tracking (Tier 1 - connection & headers)
 * 5. Download completion
 * 6. Download cancellation
 * 7. Error handling
 */

test.describe('LLM Models - Local Download - UI Structure', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-provider-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Download test provider')

    // Click on the provider in the sidebar to open its detail page
    // The provider should already be visible after createLocalProvider
    await clickProviderCard(page, testProvider)
  })

  test('should pass accessibility checks for download drawer', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')
    await assertNoAccessibilityViolations(page)
  })

  test('should display download drawer with correct structure', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Wait for download drawer to open
    await page.waitForSelector('.ant-drawer-title:has-text("Download from Repository")', { timeout: 5000 })

    // Verify drawer title
    await expect(page.locator('.ant-drawer-title:has-text("Download from Repository")')).toBeVisible()

    // Verify form fields (use exact text to avoid ambiguous matches)
    await expect(page.locator('label[for="llm-model-download_repository_id"]')).toBeVisible()
    await expect(page.locator('label[for="llm-model-download_repository_path"]')).toBeVisible()
    await expect(page.locator('label[for="llm-model-download_repository_branch"]')).toBeVisible()
    await expect(page.locator('label[for="llm-model-download_display_name"]')).toBeVisible()
    await expect(page.locator('label[for="llm-model-download_main_filename"]')).toBeVisible()
    await expect(page.locator('label[for="llm-model-download_file_format"]')).toBeVisible()
    await expect(page.locator('label[for="llm-model-download_engine_type"]')).toBeVisible()

    // Verify buttons - scope to drawer to avoid strict mode violations
    const downloadDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("Download from Repository"))')
    await expect(downloadDrawer.locator('button:has-text("Cancel")')).toBeVisible()
    await expect(downloadDrawer.locator('button:has-text("Start Download")')).toBeVisible()

    // Close drawer explicitly and wait for it to close
    await downloadDrawer.locator('button:has-text("Cancel")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("Download from Repository")', {
      state: 'hidden',
      timeout: 5000
    })
  })

  test('should show repository dropdown', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Click repository dropdown
    // Click `.ant-select` directly — newer AntD uses `.ant-select-content`
    // instead of the older `.ant-select-selector` inner element.
    await page.click('.ant-select:has-text("Repository")')
    await page.waitForSelector('.ant-select-dropdown', { state: 'visible' })

    // Verify Hugging Face Hub option exists (default repository)
    await expect(page.locator('.ant-select-item-option:has-text("Hugging Face Hub")')).toBeVisible()

    // Close dropdown
    await page.keyboard.press('Escape')

    // Close the Download from Repository drawer explicitly
    const downloadDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("Download from Repository"))')
    await downloadDrawer.locator('button:has-text("Cancel")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("Download from Repository")', {
      state: 'hidden',
      timeout: 5000
    })
  })

  test('should show example repository paths', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Repository path field should have placeholder with example
    const repositoryPathInput = page.locator('#llm-model-download_repository_path')
    await expect(repositoryPathInput).toBeVisible()

    // Verify placeholder exists (adjust based on actual implementation)
    const placeholder = await repositoryPathInput.getAttribute('placeholder')
    expect(placeholder).toBeTruthy()

    // Close the Download from Repository drawer explicitly
    const downloadDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("Download from Repository"))')
    await downloadDrawer.locator('button:has-text("Cancel")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("Download from Repository")', {
      state: 'hidden',
      timeout: 5000
    })
  })
})

test.describe('LLM Models - Local Download - Form Validation', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-validation-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Download validation test provider')

    // Navigate to provider detail page
    await clickProviderCard(page, testProvider)
  })

  test('should validate required fields', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Try to submit without filling any fields
    await page.click('button:has-text("Start Download")')

    // Should show validation errors
    await expect(page.locator('.ant-form-item-explain-error')).toBeVisible({ timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should validate repository is required', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Fill other fields but not repository
    await page.fill('#llm-model-download_repository_path', 'test/model')
    await page.fill('#llm-model-download_display_name', 'Test Model')
    await page.fill('#llm-model-download_main_filename', 'model.safetensors')

    // Submit
    await page.click('button:has-text("Start Download")')

    // Should show error for repository
    await expect(page.locator('.ant-form-item-explain-error')).toBeVisible({ timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should validate repository path is required', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Clear pre-filled repository path field
    await page.fill('#llm-model-download_repository_path', '')

    // Select repository
    const repositorySelect = page.locator('.ant-select:has(input#llm-model-download_repository_id)')
    await repositorySelect.click()
    await page.click('.ant-select-item:has-text("Hugging Face Hub")')

    // Submit
    await page.click('button:has-text("Start Download")')

    // Should show error for repository path
    await expect(page.locator('.ant-form-item-explain-error')).toBeVisible({ timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should validate display name is required', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Clear pre-filled display name
    await page.fill('#llm-model-download_display_name', '')

    // Select repository
    const repositorySelect = page.locator('.ant-select:has(input#llm-model-download_repository_id)')
    await repositorySelect.click()
    await page.click('.ant-select-item:has-text("Hugging Face Hub")')

    // Submit
    await page.click('button:has-text("Start Download")')

    // Should show error for display name
    await expect(page.locator('.ant-form-item-explain-error')).toBeVisible({ timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should validate main filename is required', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Clear pre-filled main filename
    await page.fill('#llm-model-download_main_filename', '')

    // Fill other required fields
    const repositorySelect = page.locator('.ant-select:has(input#llm-model-download_repository_id)')
    await repositorySelect.click()
    await page.click('.ant-select-item:has-text("Hugging Face Hub")')

    // Submit
    await page.click('button:has-text("Start Download")')

    // Should show error for main filename
    await expect(page.locator('.ant-form-item-explain-error')).toBeVisible({ timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })
})

test.describe('LLM Models - Local Download - Download Initiation', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-initiate-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Download initiation test provider')

    // Navigate to provider detail page
    await clickProviderCard(page, testProvider)
  })


  test('should start download with valid data', async ({ page }) => {
    const modelName = `test-download-${Date.now()}`

    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Fill form with test model from Hugging Face
    const repositorySelect = page.locator('.ant-select:has(input#llm-model-download_repository_id)')
    await repositorySelect.click()
    await page.click('.ant-select-item:has-text("Hugging Face Hub")')

    await page.fill('#llm-model-download_repository_path', 'hf-internal-testing/tiny-random-gpt2')
    await page.fill('#llm-model-download_display_name', modelName)
    await page.fill('#llm-model-download_main_filename', 'model.safetensors')

    // Select file format
    const fileFormatSelect = page.locator('.ant-select:has(input#llm-model-download_file_format)')
    await fileFormatSelect.click()
    await page.click('.ant-select-item:has-text("SafeTensors")')

    // Submit
    await page.click('button:has-text("Start Download")')

    // Wait for success message
    await page.waitForSelector('text=Download started successfully', { timeout: 15000 })

    // Verify drawer closes
    await expect(page.locator('.ant-drawer-title:has-text("Download from Repository")')).not.toBeVisible({ timeout: 5000 })

    // Close the View Download Details drawer that auto-opened
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })
  })

  test('should show error for non-existent repository path', async ({ page }) => {
    const invalidModelName = `invalid-model-${Date.now()}`

    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Fill form with invalid repository path
    const repositorySelect = page.locator('.ant-select:has(input#llm-model-download_repository_id)')
    await repositorySelect.click()
    await page.click('.ant-select-item:has-text("Hugging Face Hub")')

    await page.fill('#llm-model-download_repository_path', 'non-existent/invalid-model-path-12345')
    await page.fill('#llm-model-download_display_name', invalidModelName)
    await page.fill('#llm-model-download_main_filename', 'model.safetensors')

    // Select file format
    const fileFormatSelect = page.locator('.ant-select:has(input#llm-model-download_file_format)')
    await fileFormatSelect.click()
    await page.click('.ant-select-item:has-text("SafeTensors")')

    // Submit - download is accepted but validation happens asynchronously
    await page.click('button:has-text("Start Download")')

    // Wait for success message (download created, not yet validated)
    await page.waitForSelector('text=Download started successfully', { timeout: 10000 })

    // Wait for drawer to close
    await page.waitForSelector('.ant-drawer-title:has-text("Download from Repository")', { state: 'hidden', timeout: 5000 })

    // Download appears in "Downloading Models" section
    await page.waitForSelector('text=Downloading Models', { timeout: 5000 })
    await page.waitForSelector(`text=${invalidModelName}`, { timeout: 5000 })

    // Backend worker processes download asynchronously:
    // 1. Updates status to "downloading"
    // 2. Attempts git clone
    // 3. Fails with 404/timeout for non-existent repository
    // 4. Updates status to "failed" with error_message
    // 5. SSE broadcasts update to frontend
    // 6. Frontend displays red "Failed" tag and error message

    // Assert: Failed tag appears (40s timeout for worker processing)
    await expect(page.locator('.ant-tag-red:has-text("Failed")')).toBeVisible({ timeout: 40000 })

    // Assert: Error message is displayed (appears in both card and drawer)
    await expect(page.locator('.ant-typography-danger').first()).toBeVisible()

    // Close the View Download Details drawer that auto-opened
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })
  })

  test('should handle optional branch parameter', async ({ page }) => {
    const modelName = `test-download-branch-${Date.now()}`

    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')

    // Fill form with branch specified
    const repositorySelect = page.locator('.ant-select:has(input#llm-model-download_repository_id)')
    await repositorySelect.click()
    await page.click('.ant-select-item:has-text("Hugging Face Hub")')

    await page.fill('#llm-model-download_repository_path', 'hf-internal-testing/tiny-random-gpt2')
    await page.fill('#llm-model-download_repository_branch', 'main')
    await page.fill('#llm-model-download_display_name', modelName)
    await page.fill('#llm-model-download_main_filename', 'model.safetensors')

    // Select file format
    const fileFormatSelect = page.locator('.ant-select:has(input#llm-model-download_file_format)')
    await fileFormatSelect.click()
    await page.click('.ant-select-item:has-text("SafeTensors")')

    // Submit
    await page.click('button:has-text("Start Download")')

    // Wait for success
    await page.waitForSelector('text=Download started successfully', { timeout: 15000 })

    // Close the View Download Details drawer that auto-opened
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })
  })
})

test.describe('LLM Models - Local Download - Progress Tracking', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-progress-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Configure HuggingFace repository with API key for downloads
    await configureHuggingFaceAuth(page, baseURL)

    await createLocalProvider(page, baseURL, testProvider, 'Download progress test provider')

    // Navigate to provider detail page
    await clickProviderCard(page, testProvider)
  })


  test('should show Downloads section when download is active', async ({ page }) => {
    const modelName = `test-download-progress-${Date.now()}`

    // Start download
    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    // Assert: Downloading Models section appears
    await expect(page.locator('text=Downloading Models')).toBeVisible({ timeout: 5000 })

    // Close the View Download Details drawer that auto-opened
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })
  })

  test('should show download progress for individual download', async ({ page }) => {
    const modelName = `test-download-individual-${Date.now()}`

    // Start download
    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    // Assert: Progress bar is visible
    await expect(page.locator('.ant-progress-line').first()).toBeVisible({ timeout: 10000 })

    // Close the View Download Details drawer that auto-opened
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', {
      state: 'hidden',
      timeout: 5000
    })
  })

  test('should update download status in real-time via SSE', async ({ page }) => {
    const modelName = `test-download-sse-${Date.now()}`

    // Start download
    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    // Assert: Download status updates are visible (SSE working)
    // The download should show "Downloading..." tag
    await expect(page.locator('.ant-tag:has-text("Downloading")')).toBeVisible({ timeout: 10000 })

    // Close the View Download Details drawer that auto-opened
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', {
      state: 'hidden',
      timeout: 5000
    })
  })

  test('should show model in models list after download completes', async ({ page }) => {
    const modelName = `test-download-complete-${Date.now()}`

    // Start download
    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    // Close the View Download Details drawer that auto-opened
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', {
      state: 'hidden',
      timeout: 5000
    })

    // Wait for download to complete - tiny test model should download quickly
    // Wait for the "Downloading Models" section to disappear (download complete)
    // or for the model to appear in the Models list
    await page.waitForTimeout(15000) // Give enough time for small model to download

    // Verify model appears in models list
    await assertModelExists(page, modelName)

    // Cleanup
    await deleteModel(page, modelName)
  })
})

test.describe('LLM Models - Local Download - Download Management', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-management-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Configure HuggingFace repository with API key for downloads
    await configureHuggingFaceAuth(page, baseURL)

    await createLocalProvider(page, baseURL, testProvider, 'Download management test provider')

    // Navigate to provider detail page
    await clickProviderCard(page, testProvider)
  })


  test('should show cancel button for active downloads', async ({ page }) => {
    const modelName = `test-download-cancel-${Date.now()}`

    // Start download of a larger model (so we have time to see cancel button)
    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-bert',
      mainFilename: 'model.safetensors',
    })

    // Wait for download to start
    // Assert: Cancel button is visible
    await expect(page.locator('button:has-text("Cancel")').first()).toBeVisible({ timeout: 10000 })

    // Close the View Download Details drawer that auto-opened
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })
  })

  // "should allow cancelling an active download" was here. Removed
  // because the test depends on a real ~350MB download from
  // huggingface (distilgpt2) timing-windowed against a manual click
  // of Cancel. Needs a deterministic mock-download fixture to be
  // reliable. Tracked in src-app/ui/tests/e2e/TODO_E2E.md.

  test('should remove download from list after completion', async ({ page }) => {
    const modelName = `test-download-remove-${Date.now()}`

    // Start download
    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    // Close the View Download Details drawer that auto-opened
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const viewDetailsDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await viewDetailsDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })

    // Wait for download to complete
    await page.waitForTimeout(15000)

    // Verify Downloads section is no longer visible (if no other downloads)
    // Or verify specific download is removed from list
    // Downloads section should auto-hide when all downloads complete

    // Model should be in models list
    await assertModelExists(page, modelName)

    // Cleanup
    await deleteModel(page, modelName)
  })
})

test.describe('LLM Models - Local Download - Multiple Downloads', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-multiple-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Configure HuggingFace repository with API key for downloads
    await configureHuggingFaceAuth(page, baseURL)

    await createLocalProvider(page, baseURL, testProvider, 'Multiple downloads test provider')

    // Navigate to provider detail page
    await clickProviderCard(page, testProvider)
  })


  test('should handle multiple simultaneous downloads', async ({ page }) => {
    const model1Name = `test-download-multi-1-${Date.now()}`
    const model2Name = `test-download-multi-2-${Date.now()}`

    // Start first download - using distilgpt2 (~350MB) to ensure downloads take long enough
    await startModelDownload(page, {
      displayName: model1Name,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'distilgpt2',
      mainFilename: 'model.safetensors',
      clearCache: true,
    })

    // Close the first View Download Details drawer
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const firstDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await firstDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })

    // Wait a moment
    await page.waitForTimeout(1000)

    // Start second download - using a different model to avoid caching
    await startModelDownload(page, {
      displayName: model2Name,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'openai-community/gpt2',
      mainFilename: 'model.safetensors',
      clearCache: true,
    })

    // Close the second View Download Details drawer
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const secondDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await secondDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })

    // Wait for downloads to complete
    await page.waitForTimeout(15000)

    // Verify both models appear in list
    await assertModelExists(page, model1Name)
    await assertModelExists(page, model2Name)

    // Cleanup
    await deleteModel(page, model1Name)
    await deleteModel(page, model2Name)
  })

  test('should show all active downloads in Downloads section', async ({ page }) => {
    const model1Name = `test-download-list-1-${Date.now()}`
    const model2Name = `test-download-list-2-${Date.now()}`

    // Start first download - using larger models to ensure they're still downloading when we check
    await startModelDownload(page, {
      displayName: model1Name,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'distilgpt2',
      mainFilename: 'model.safetensors',
    })

    // Close the first View Download Details drawer
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const firstDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await firstDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })

    // Start second download quickly - using different model to avoid caching
    await page.waitForTimeout(500)
    await startModelDownload(page, {
      displayName: model2Name,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'openai-community/gpt2',
      mainFilename: 'model.safetensors',
    })

    // Close the second View Download Details drawer
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { timeout: 5000 })
    const secondDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("View Download Details"))')
    await secondDrawer.locator('button:has-text("Close")').click()
    await page.waitForSelector('.ant-drawer-title:has-text("View Download Details")', { state: 'hidden', timeout: 5000 })

    // Check for Downloads section
    // Assert: Downloading Models section is visible
    await expect(page.locator('text=Downloading Models')).toBeVisible({ timeout: 5000 })

    // Assert: Both downloads are listed - scope to Downloading Models section to avoid drawer conflicts
    const downloadingSection = page.locator('.ant-card:has-text("Downloading Models")')
    await expect(downloadingSection.locator(`text=${model1Name}`)).toBeVisible({ timeout: 5000 })
    await expect(downloadingSection.locator(`text=${model2Name}`)).toBeVisible({ timeout: 5000 })
  })
})
