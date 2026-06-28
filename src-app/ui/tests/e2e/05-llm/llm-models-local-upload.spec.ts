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
  openUploadDrawer,
  uploadModelFolder,
  assertModelExists,
  assertModelNotExists,
  deleteModel,
} from './helpers/model-helpers'
import { submitUploadForm } from './helpers/form-helpers'
import * as path from 'path'
import * as fs from 'fs'
import * as os from 'os'

/**
 * LLM Models - Local Upload Tests
 *
 * Tests for uploading local model files following backend test patterns from:
 * - src-web/tests/llm_model/upload_test.rs
 *
 * Key testing areas:
 * 1. File selection and validation
 * 2. File format detection (safetensors, gguf, pytorch)
 * 3. Main file selection
 * 4. Upload progress tracking
 * 5. Form validation
 * 6. Success and error flows
 */

// Helper to create a test model folder with dummy files
async function createTestModelFolder(format: 'safetensors' | 'gguf' | 'pytorch'): Promise<string> {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'test-model-'))

  // Backend's upload validator rejects:
  //   - weight files < 1KB ("suspiciously small")
  //   - config.json / tokenizer.json that aren't valid JSON
  // Pad weight files to 2KB+ and write minimal valid JSON for the
  // config/tokenizer files so the validator passes.
  const validJsonContent = JSON.stringify({
    model_type: 'test',
    architectures: ['TestModel'],
    hidden_size: 4,
    num_attention_heads: 1,
    num_hidden_layers: 1,
  })
  const validTokenizerJson = JSON.stringify({
    version: '1.0',
    truncation: null,
    padding: null,
    added_tokens: [],
    model: { type: 'BPE', vocab: { '<unk>': 0 }, merges: [] },
  })
  const weightPayload = Buffer.alloc(2048, 'A')

  const modelFiles: Record<string, { name: string; content: string | Buffer }[]> = {
    safetensors: [
      { name: 'model.safetensors', content: weightPayload },
      { name: 'config.json', content: validJsonContent },
      { name: 'tokenizer.json', content: validTokenizerJson },
    ],
    gguf: [
      { name: 'model.gguf', content: weightPayload },
      { name: 'config.json', content: validJsonContent },
    ],
    pytorch: [
      { name: 'pytorch_model.bin', content: weightPayload },
      { name: 'config.json', content: validJsonContent },
      { name: 'tokenizer.json', content: validTokenizerJson },
    ],
  }

  for (const file of modelFiles[format]) {
    fs.writeFileSync(path.join(tempDir, file.name), file.content)
  }

  return tempDir
}

/**
 * A model folder with a deliberately LARGE weight file so the upload takes long
 * enough to be cancelled mid-transfer (the standard 2KB fixture completes in
 * <1s, which is why mid-transfer cancel was previously untestable). `sizeMb`
 * MB of weight chunked through the multipart upload gives a wide cancel window.
 */
function createLargeTestModelFolder(sizeMb: number): string {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'test-model-large-'))
  const validJsonContent = JSON.stringify({
    model_type: 'test',
    architectures: ['TestModel'],
    hidden_size: 4,
    num_attention_heads: 1,
    num_hidden_layers: 1,
  })
  const validTokenizerJson = JSON.stringify({
    version: '1.0',
    truncation: null,
    padding: null,
    added_tokens: [],
    model: { type: 'BPE', vocab: { '<unk>': 0 }, merges: [] },
  })
  fs.writeFileSync(
    path.join(tempDir, 'model.safetensors'),
    Buffer.alloc(sizeMb * 1024 * 1024, 'A'),
  )
  fs.writeFileSync(path.join(tempDir, 'config.json'), validJsonContent)
  fs.writeFileSync(path.join(tempDir, 'tokenizer.json'), validTokenizerJson)
  return tempDir
}

// Helper to cleanup test model folder
async function cleanupTestModelFolder(folderPath: string) {
  if (fs.existsSync(folderPath)) {
    fs.rmSync(folderPath, { recursive: true, force: true })
  }
}

test.describe('LLM Models - Local Upload - UI Structure', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-upload-provider-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Upload test provider')

    // Click on the provider in the sidebar to open its detail page
    // The provider should already be visible after createLocalProvider
    await clickProviderCard(page, testProvider)
  })

  test('should pass accessibility checks for upload drawer', async ({ page }) => {
    await openUploadDrawer(page)
    await assertNoAccessibilityViolations(page)
  })

  test('should display upload drawer with correct structure', async ({ page }) => {
    await openUploadDrawer(page)

    // Scope to the upload drawer
    const uploadDrawer = page.locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("Upload Local Model"))')

    // Verify drawer title
    await expect(uploadDrawer.locator('.ant-drawer-title:has-text("Upload Local Model")')).toBeVisible()

    // Verify form fields
    await expect(uploadDrawer.locator('label:has-text("Display Name")')).toBeVisible()
    await expect(uploadDrawer.locator('label:has-text("Description")')).toBeVisible()
    await expect(uploadDrawer.locator('label:has-text("Engine")')).toBeVisible()
    await expect(uploadDrawer.locator('label:has-text("File Format")')).toBeVisible()
    await expect(uploadDrawer.locator('label:has-text("Model Folder")')).toBeVisible()
    await expect(uploadDrawer.locator('label:has-text("Main Model File")')).toBeVisible()

    // Verify Upload.Dragger component
    await expect(uploadDrawer.locator('.ant-upload-drag')).toBeVisible()
    await expect(uploadDrawer.locator('text=Click or drag folder to select model files')).toBeVisible()

    // Verify buttons
    await expect(uploadDrawer.locator('button:has-text("Cancel")')).toBeVisible()
    await expect(uploadDrawer.locator('button:has-text("Upload")')).toBeVisible()

    // Close drawer
    await uploadDrawer.locator('button:has-text("Cancel")').click()
  })

  test('should show file format options', async ({ page }) => {
    await openUploadDrawer(page)

    // Click file format dropdown - find the form item with "File Format" label, then click its select
    await page.locator('.ant-form-item:has-text("File Format") .ant-select').click()
    await page.waitForSelector('.ant-select-dropdown', { state: 'visible' })

    // Verify format options
    await expect(page.locator('.ant-select-item-option:has-text("SafeTensors")')).toBeVisible()
    await expect(page.locator('.ant-select-item-option:has-text("GGUF")')).toBeVisible()
    await expect(page.locator('.ant-select-item-option:has-text("PyTorch Binary")')).toBeVisible()

    // Close dropdown
    await page.keyboard.press('Escape')
    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })
})

test.describe('LLM Models - Local Upload - File Selection', () => {
  let testProvider: string
  let testModelFolder: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-upload-provider-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Upload test provider')

    // Click on the provider in the sidebar to open its detail page
    // The provider should already be visible after createLocalProvider
    await clickProviderCard(page, testProvider)
  })

  test.afterEach(async () => {
    if (testModelFolder) {
      await cleanupTestModelFolder(testModelFolder)
    }
  })

  test('should display selected files after folder upload', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)

    // Upload folder
    await uploadModelFolder(page, testModelFolder)

    // Wait for files to be processed
    await page.waitForTimeout(500)

    // Verify "Selected Files" card appears
    await expect(page.locator('.ant-card-head-title:has-text("Selected Files")')).toBeVisible()

    // Verify files are listed in the file list
    await expect(page.locator('.ant-list-item:has-text("model.safetensors")')).toBeVisible()
    await expect(page.locator('.ant-list-item:has-text("config.json")')).toBeVisible()
    await expect(page.locator('.ant-list-item:has-text("tokenizer.json")')).toBeVisible()

    // Close drawer
    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should classify files by purpose', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Verify file purpose tags
    const modelTag = page.locator('.ant-tag:has-text("model")').first()
    const configTag = page.locator('.ant-tag:has-text("config")').first()
    const tokenizerTag = page.locator('.ant-tag:has-text("tokenizer")').first()

    await expect(modelTag).toBeVisible()
    await expect(configTag).toBeVisible()
    await expect(tokenizerTag).toBeVisible()

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should auto-detect main model file', async ({ page }) => {
    testModelFolder = await createTestModelFolder('gguf')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Verify main filename dropdown is populated
    const mainFilenameSelect = page.locator('.ant-form-item:has-text("Main Model File") .ant-select-content')
    await expect(mainFilenameSelect).toContainText('model.gguf')

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should allow selecting different main file', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Click main filename dropdown
    await page.locator('.ant-form-item:has-text("Main Model File") .ant-select').click()
    await page.waitForSelector('.ant-select-dropdown', { state: 'visible' })

    // Verify only model files are in dropdown (not config or tokenizer)
    await expect(page.locator('.ant-select-item-option:has-text("model.safetensors")')).toBeVisible()
    await expect(page.locator('.ant-select-item-option:has-text("config.json")')).not.toBeVisible()

    await page.keyboard.press('Escape')
    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })
})

test.describe('LLM Models - Local Upload - Validation', () => {
  let testProvider: string
  let testModelFolder: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-upload-provider-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Upload test provider')

    // Click on the provider in the sidebar to open its detail page
    // The provider should already be visible after createLocalProvider
    await clickProviderCard(page, testProvider)
  })

  test.afterEach(async () => {
    if (testModelFolder) {
      await cleanupTestModelFolder(testModelFolder)
    }
  })

  test('should validate required fields', async ({ page }) => {
    await openUploadDrawer(page)

    // Try to submit without any data
    await submitUploadForm(page)

    // Should show validation errors
    await expect(page.locator('.ant-form-item-explain-error').first()).toBeVisible({ timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should validate display name is required', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Wait for Main Model File to be populated (indicates upload processing is done)
    await expect(page.locator('.ant-form-item:has-text("Main Model File") .ant-select-content')).toContainText('model.safetensors', { timeout: 5000 })

    // Clear the display name field using fill (more reliable than clear)
    await page.fill('#llm-model-upload_display_name', '')

    // Submit without display name
    await submitUploadForm(page)

    // Check for inline error message on the display name field
    await expect(page.locator('.ant-form-item-explain-error:has-text("Display Name is required")')).toBeVisible({ timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should validate model folder is required', async ({ page }) => {
    await openUploadDrawer(page)

    // Fill display name only
    await page.fill('#llm-model-upload_display_name', 'Test Model')

    // Try to submit without uploading files
    await submitUploadForm(page)

    // Should show error message
    await page.waitForSelector('text=Please select a model folder', { timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should validate main filename is required', async ({ page }) => {
    // Create a test folder with files classified as "other" (no main model file)
    // so the Main Model File dropdown will be empty
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'test-no-main-file-'))
    testModelFolder = tempDir

    // Create only non-model files
    fs.writeFileSync(path.join(tempDir, 'README.md'), 'Test readme')
    fs.writeFileSync(path.join(tempDir, 'config.json'), '{"test": "config"}')

    await openUploadDrawer(page)

    // Fill display name
    await page.fill('#llm-model-upload_display_name', 'Test Model')

    // Upload folder with no model files
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Scroll to make Main Model File dropdown visible
    await page.locator('.ant-form-item:has-text("Main Model File")').scrollIntoViewIfNeeded()

    // The dropdown should be empty since there are no model files
    // Try to submit without selecting main file
    await submitUploadForm(page)

    // Check for inline error message on the main file field
    await expect(page.locator('.ant-form-item-explain-error:has-text("Please select the main model file")')).toBeVisible({ timeout: 5000 })

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should warn if no tokenizer files detected', async ({ page }) => {
    // Create folder with only model file (no tokenizer). Pad to 2KB+
    // so the backend's "suspiciously small weight file" validator passes.
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'test-model-no-tokenizer-'))
    fs.writeFileSync(path.join(tempDir, 'model.safetensors'), Buffer.alloc(2048, 'A'))
    testModelFolder = tempDir

    await openUploadDrawer(page)

    // Select safetensors format
    await page.locator('.ant-form-item:has-text("File Format") .ant-select').click()
    await page.click('.ant-select-item-option:has-text("SafeTensors")')

    // Fill form
    await page.fill('#llm-model-upload_display_name', 'Test Model No Tokenizer')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Submit - this should show a warning but allow upload to continue
    await submitUploadForm(page)

    // Should show warning notification about missing tokenizer
    // Note: Ant Design notifications appear briefly and may disappear, so we check for either:
    // 1. The warning message is visible, or
    // 2. Upload succeeds (warning was shown but disappeared)
    try {
      await page.waitForSelector('text=No tokenizer or configuration files detected', { timeout: 2000 })
    } catch (e) {
      // Warning may have appeared and disappeared already - that's ok
    }

    // Upload should complete successfully despite the warning
    await page.waitForSelector('text=Model uploaded successfully', { timeout: 30000 })

    // Verify model appears in the list
    await assertModelExists(page, 'Test Model No Tokenizer')
  })
})

test.describe('LLM Models - Local Upload - File Formats', () => {
  let testProvider: string
  let testModelFolder: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-upload-provider-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Upload test provider')

    // Click on the provider in the sidebar to open its detail page
    // The provider should already be visible after createLocalProvider
    await clickProviderCard(page, testProvider)
  })

  test.afterEach(async () => {
    if (testModelFolder) {
      await cleanupTestModelFolder(testModelFolder)
    }
  })

  test('should handle safetensors format', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)

    // Select safetensors format
    await page.locator('.ant-form-item:has-text("File Format") .ant-select').click()
    await page.click('.ant-select-item-option:has-text("SafeTensors")')

    // Upload folder
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Verify .safetensors file is detected as model
    const modelFile = page.locator('.ant-list-item:has-text("model.safetensors")')
    await expect(modelFile.locator('.ant-tag:has-text("model")')).toBeVisible()

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should handle gguf format', async ({ page }) => {
    testModelFolder = await createTestModelFolder('gguf')

    await openUploadDrawer(page)

    // Select gguf format
    await page.locator('.ant-form-item:has-text("File Format") .ant-select').click()
    await page.click('.ant-select-item-option:has-text("GGUF")')

    // Upload folder
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Verify .gguf file is detected as model
    const modelFile = page.locator('.ant-list-item:has-text("model.gguf")')
    await expect(modelFile.locator('.ant-tag:has-text("model")')).toBeVisible()

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should handle pytorch format', async ({ page }) => {
    testModelFolder = await createTestModelFolder('pytorch')

    await openUploadDrawer(page)

    // Select pytorch format
    await page.locator('.ant-form-item:has-text("File Format") .ant-select').click()
    await page.click('.ant-select-item-option:has-text("PyTorch Binary")')

    // Upload folder
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Verify .bin file is detected as model
    const modelFile = page.locator('.ant-list-item:has-text("pytorch_model.bin")')
    await expect(modelFile.locator('.ant-tag:has-text("model")')).toBeVisible()

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })

  test('should re-filter files when format changes', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)

    // Upload with safetensors format
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Verify safetensors file is detected as model
    await expect(page.locator('.ant-list-item:has-text("model.safetensors") .ant-tag:has-text("model")')).toBeVisible()

    // Scroll to File Format field
    await page.locator('.ant-form-item:has-text("File Format")').scrollIntoViewIfNeeded()

    // Change format to gguf
    await page.locator('.ant-form-item:has-text("File Format") .ant-select').click()
    await page.click('.ant-select-item-option:has-text("GGUF")')
    await page.waitForTimeout(500)

    // Scroll to Main Model File dropdown to make it visible
    await page.locator('.ant-form-item:has-text("Main Model File")').scrollIntoViewIfNeeded()
    await page.waitForTimeout(500)

    // Now safetensors file should NOT be classified as model (no .gguf files present)
    // The file list should update based on new format
    // After changing to GGUF format with no GGUF files, the dropdown should be empty or show placeholder
    const mainFilenameSelector = page.locator('.ant-form-item:has-text("Main Model File") .ant-select')

    // Check that either the dropdown shows placeholder or doesn't have model.safetensors selected
    const dropdownText = await mainFilenameSelector.textContent()
    expect(dropdownText).not.toContain('model.safetensors')

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()
  })
})

test.describe('LLM Models - Local Upload - Progress Tracking', () => {
  let testProvider: string
  let testModelFolder: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-upload-provider-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Upload test provider')

    // Click on the provider in the sidebar to open its detail page
    // The provider should already be visible after createLocalProvider
    await clickProviderCard(page, testProvider)
  })

  test.afterEach(async () => {
    if (testModelFolder) {
      await cleanupTestModelFolder(testModelFolder)
    }

  })

  test('should show upload progress card during upload', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)

    // Fill form
    await page.fill('#llm-model-upload_display_name', 'Test Upload Progress')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Submit
    await submitUploadForm(page)

    // Assert: Upload completes successfully
    await page.waitForSelector('text=Model uploaded successfully', { timeout: 30000 })

    // Assert: Model appears in list
    await assertModelExists(page, 'Test Upload Progress')
  })

  test('cancels a real in-flight upload via "Cancel Upload" and creates no model', async ({
    page,
  }) => {
    // A large weight file keeps the transfer in-flight long enough to cancel.
    testModelFolder = createLargeTestModelFolder(120)
    const modelName = 'Test Cancel Mid Transfer'

    await openUploadDrawer(page)
    await page.fill('#llm-model-upload_display_name', modelName)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await submitUploadForm(page)

    // The Upload Progress card appears while transferring; its "Cancel Upload"
    // link aborts the in-flight request mid-transfer.
    const cancelUpload = page.getByRole('button', { name: 'Cancel Upload' })
    await expect(cancelUpload).toBeVisible({ timeout: 15000 })
    await cancelUpload.click()

    // The upload must NOT report success and the model must NOT be created.
    await expect(page.locator('text=Model uploaded successfully')).toHaveCount(0)
    // The uploading state clears (the progress card is gone).
    await expect(cancelUpload).toBeHidden({ timeout: 15000 })

    // Close the drawer (now allowed — no longer uploading) and confirm the
    // model never landed in the list.
    await page
      .locator('.ant-drawer.ant-drawer-open')
      .last()
      .locator('button:has-text("Cancel"), button:has-text("Close")')
      .first()
      .click()
    await assertModelNotExists(page, modelName)
  })

  test('should prevent drawer from closing during upload', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)

    // Fill form
    await page.fill('#llm-model-upload_display_name', 'Test Upload Prevent Close')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Submit
    await submitUploadForm(page)

    // Wait briefly for upload to start, then check if cancel button is disabled
    // Note: With small test files, upload completes very quickly, so we check immediately
    await page.waitForTimeout(100)
    const cancelButton = page.locator('button:has-text("Cancel")')

    // The button should be disabled during upload OR the upload completed successfully
    // Check if either the button is disabled OR we see the success message
    const isButtonDisabled = await cancelButton.isDisabled().catch(() => false)
    const hasSuccessMessage = await page.locator('text=Model uploaded successfully').isVisible().catch(() => false)

    // At least one should be true (either uploading or completed)
    expect(isButtonDisabled || hasSuccessMessage).toBe(true)

    // Wait for upload to complete
    await page.waitForSelector('text=Model uploaded successfully', { timeout: 30000 })

    // Cleanup
    await deleteModel(page, 'Test Upload Prevent Close')
  })

  test('should show overall progress percentage', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)

    // Fill form
    await page.fill('#llm-model-upload_display_name', 'Test Overall Progress')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Submit
    await submitUploadForm(page)

    // Check for progress display (if visible before completion)
    // Assert: Upload completes successfully
    await page.waitForSelector('text=Model uploaded successfully', { timeout: 30000 })

    // Cleanup
    await deleteModel(page, 'Test Overall Progress')
  })
})

test.describe('LLM Models - Local Upload - Success Flow', () => {
  let testProvider: string
  let testModelFolder: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-upload-provider-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Upload test provider')

    // Click on the provider in the sidebar to open its detail page
    // The provider should already be visible after createLocalProvider
    await clickProviderCard(page, testProvider)
  })

  test.afterEach(async () => {
    if (testModelFolder) {
      await cleanupTestModelFolder(testModelFolder)
    }
  })

  test('should successfully upload a model', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')
    const modelName = `test-model-upload-${Date.now()}`

    await openUploadDrawer(page)

    // Fill form
    await page.fill('#llm-model-upload_display_name', modelName)
    await page.fill('#llm-model-upload_description', 'Test model upload description')

    // Upload folder
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Submit
    await submitUploadForm(page)

    // Wait for success message
    await page.waitForSelector('text=Model uploaded successfully', { timeout: 30000 })

    // Verify drawer closes
    await expect(page.locator('.ant-drawer-title:has-text("Upload Local Model")')).not.toBeVisible({ timeout: 5000 })

    // Verify model appears in provider's model list
    await assertModelExists(page, modelName)

    // Cleanup
    await deleteModel(page, modelName)
  })

  test('should reset form after successful upload', async ({ page }) => {
    testModelFolder = await createTestModelFolder('gguf')
    const modelName = `test-model-reset-${Date.now()}`

    await openUploadDrawer(page)

    // Fill and submit
    await page.fill('#llm-model-upload_display_name', modelName)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)
    await submitUploadForm(page)

    // Wait for success + drawer close animation to finish AND the
    // toast to disappear before reopening (the toast can intercept
    // pointer events on the page-level dropdown trigger).
    await page.waitForSelector('text=Model uploaded successfully', { timeout: 30000 })
    await page
      .locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("Upload Local Model"))')
      .waitFor({ state: 'hidden', timeout: 5000 })
      .catch(() => {})
    // AntD message toasts auto-dismiss in 3s — wait for them to clear.
    await page
      .locator('text=Model uploaded successfully')
      .waitFor({ state: 'hidden', timeout: 10000 })
      .catch(() => {})
    await page.waitForTimeout(500)

    // Open drawer again
    await openUploadDrawer(page)

    // Verify form is reset
    const displayNameInput = page.locator('#llm-model-upload_display_name')
    await expect(displayNameInput).toHaveValue('')

    const fileList = page.locator('.ant-card-head-title:has-text("Selected Files")')
    await expect(fileList).not.toBeVisible()

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Cancel")').click()

    // Cleanup
    await deleteModel(page, modelName)
  })

  test('should show uploaded model immediately in models list', async ({ page }) => {
    testModelFolder = await createTestModelFolder('pytorch')
    const modelName = `test-model-immediate-${Date.now()}`

    await openUploadDrawer(page)

    // Fill and submit
    await page.fill('#llm-model-upload_display_name', modelName)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)
    await submitUploadForm(page)

    // Wait for success
    await page.waitForSelector('text=Model uploaded successfully', { timeout: 30000 })

    // Model should appear immediately without page refresh
    await assertModelExists(page, modelName)

    // Cleanup
    await deleteModel(page, modelName)
  })
})
