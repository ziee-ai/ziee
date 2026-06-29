import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
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
 */

// ---- local helpers (testid-based) ----

async function selectUploadFormat(page: Page, value: string) {
  await byTestId(page, 'llm-file-format-select').click()
  await byTestId(page, `llm-file-format-select-opt-${value}`).click()
}

async function cancelUploadForm(page: Page) {
  await byTestId(page, 'llm-upload-drawer-cancel-btn').click()
}

async function expectUploadSucceeded(page: Page) {
  // The upload drawer closes (its form unmounts) once the upload succeeds.
  await byTestId(page, 'llm-model-upload-form').waitFor({ state: 'hidden', timeout: 30000 })
}

// Helper to create a test model folder with dummy files
async function createTestModelFolder(format: 'safetensors' | 'gguf' | 'pytorch'): Promise<string> {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'test-model-'))

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
 * enough to be cancelled mid-transfer.
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
    await clickProviderCard(page, testProvider)
  })

  test('should pass accessibility checks for upload drawer', async ({ page }) => {
    await openUploadDrawer(page)
    await assertNoAccessibilityViolations(page)
  })

  test('should display upload drawer with correct structure', async ({ page }) => {
    await openUploadDrawer(page)

    // Verify form fields + controls are present.
    await expect(byTestId(page, 'llm-param-display_name')).toBeVisible()
    await expect(byTestId(page, 'llm-param-description')).toBeVisible()
    await expect(byTestId(page, 'llm-engine-type-select')).toBeVisible()
    await expect(byTestId(page, 'llm-file-format-select')).toBeVisible()
    await expect(byTestId(page, 'llm-upload-files')).toBeVisible()
    await expect(byTestId(page, 'llm-upload-main-file-select')).toBeVisible()

    // Verify buttons.
    await expect(byTestId(page, 'llm-upload-drawer-cancel-btn')).toBeVisible()
    await expect(byTestId(page, 'llm-upload-drawer-submit-btn')).toBeVisible()

    await cancelUploadForm(page)
  })

  test('should show file format options', async ({ page }) => {
    await openUploadDrawer(page)

    await byTestId(page, 'llm-file-format-select').click()

    await expect(byTestId(page, 'llm-file-format-select-opt-safetensors')).toBeVisible()
    await expect(byTestId(page, 'llm-file-format-select-opt-gguf')).toBeVisible()
    await expect(byTestId(page, 'llm-file-format-select-opt-pytorch')).toBeVisible()

    await page.keyboard.press('Escape')
    await cancelUploadForm(page)
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
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // The "Selected Files" card appears and lists each file.
    await expect(byTestId(page, 'llm-upload-selected-files-card')).toBeVisible()
    await expect(byTestId(page, 'llm-upload-file-tag-model.safetensors')).toBeVisible()
    await expect(byTestId(page, 'llm-upload-file-tag-config.json')).toBeVisible()
    await expect(byTestId(page, 'llm-upload-file-tag-tokenizer.json')).toBeVisible()

    await cancelUploadForm(page)
  })

  test('should classify files by purpose', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Each file's tag carries its classified purpose.
    await expect(byTestId(page, 'llm-upload-file-tag-model.safetensors')).toContainText('model')
    await expect(byTestId(page, 'llm-upload-file-tag-config.json')).toContainText('config')
    await expect(byTestId(page, 'llm-upload-file-tag-tokenizer.json')).toContainText('tokenizer')

    await cancelUploadForm(page)
  })

  test('should auto-detect main model file', async ({ page }) => {
    testModelFolder = await createTestModelFolder('gguf')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Main filename select is populated with the detected model file.
    await expect(byTestId(page, 'llm-upload-main-file-select')).toContainText('model.gguf')

    await cancelUploadForm(page)
  })

  test('should allow selecting different main file', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await byTestId(page, 'llm-upload-main-file-select').click()

    // Only model files appear in the dropdown (config.json must not).
    await expect(
      page.locator('[data-testid^="llm-upload-main-file-select-opt-"]').filter({ hasText: 'model.safetensors' }),
    ).toBeVisible()
    await expect(
      page.locator('[data-testid^="llm-upload-main-file-select-opt-"]').filter({ hasText: 'config.json' }),
    ).not.toBeVisible()

    await page.keyboard.press('Escape')
    await cancelUploadForm(page)
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
    await clickProviderCard(page, testProvider)
  })

  test.afterEach(async () => {
    if (testModelFolder) {
      await cleanupTestModelFolder(testModelFolder)
    }
  })

  test('should validate required fields', async ({ page }) => {
    await openUploadDrawer(page)

    await submitUploadForm(page)
    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelUploadForm(page)
  })

  test('should validate display name is required', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Wait for Main Model File to be populated (processing done).
    await expect(byTestId(page, 'llm-upload-main-file-select')).toContainText('model.safetensors', { timeout: 5000 })

    await byTestId(page, 'llm-param-display_name').fill('')
    await submitUploadForm(page)

    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelUploadForm(page)
  })

  test('should validate model folder is required', async ({ page }) => {
    await openUploadDrawer(page)

    await byTestId(page, 'llm-param-display_name').fill('Test Model')
    await submitUploadForm(page)

    // The "model folder required" error renders as a role=alert message.
    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelUploadForm(page)
  })

  test('should validate main filename is required', async ({ page }) => {
    // A folder with no model file → the Main Model File dropdown stays empty.
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'test-no-main-file-'))
    testModelFolder = tempDir
    fs.writeFileSync(path.join(tempDir, 'README.md'), 'Test readme')
    fs.writeFileSync(path.join(tempDir, 'config.json'), '{"test": "config"}')

    await openUploadDrawer(page)
    await byTestId(page, 'llm-param-display_name').fill('Test Model')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await submitUploadForm(page)
    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelUploadForm(page)
  })

  test('should warn if no tokenizer files detected', async ({ page }) => {
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'test-model-no-tokenizer-'))
    fs.writeFileSync(path.join(tempDir, 'model.safetensors'), Buffer.alloc(2048, 'A'))
    testModelFolder = tempDir

    await openUploadDrawer(page)

    await selectUploadFormat(page, 'safetensors')
    await byTestId(page, 'llm-param-display_name').fill('Test Model No Tokenizer')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // Submit — a missing-tokenizer warning may flash, but the upload proceeds.
    await submitUploadForm(page)

    // Upload completes successfully despite the warning.
    await expectUploadSucceeded(page)
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
    await selectUploadFormat(page, 'safetensors')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // .safetensors file is detected as the model file.
    await expect(byTestId(page, 'llm-upload-file-tag-model.safetensors')).toContainText('model')

    await cancelUploadForm(page)
  })

  test('should handle gguf format', async ({ page }) => {
    testModelFolder = await createTestModelFolder('gguf')

    await openUploadDrawer(page)
    await selectUploadFormat(page, 'gguf')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await expect(byTestId(page, 'llm-upload-file-tag-model.gguf')).toContainText('model')

    await cancelUploadForm(page)
  })

  test('should handle pytorch format', async ({ page }) => {
    testModelFolder = await createTestModelFolder('pytorch')

    await openUploadDrawer(page)
    await selectUploadFormat(page, 'pytorch')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await expect(byTestId(page, 'llm-upload-file-tag-pytorch_model.bin')).toContainText('model')

    await cancelUploadForm(page)
  })

  test('should re-filter files when format changes', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    // safetensors file is detected as model.
    await expect(byTestId(page, 'llm-upload-file-tag-model.safetensors')).toContainText('model')

    // Change format to gguf (no GGUF files present).
    await selectUploadFormat(page, 'gguf')
    await page.waitForTimeout(500)

    // The Main Model File select should no longer hold model.safetensors.
    const mainSelect = byTestId(page, 'llm-upload-main-file-select')
    const dropdownText = await mainSelect.textContent()
    expect(dropdownText).not.toContain('model.safetensors')

    await cancelUploadForm(page)
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
    await byTestId(page, 'llm-param-display_name').fill('Test Upload Progress')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await submitUploadForm(page)

    await expectUploadSucceeded(page)
    await assertModelExists(page, 'Test Upload Progress')
  })

  test('cancels a real in-flight upload via "Cancel Upload" and creates no model', async ({ page }) => {
    // A large weight file keeps the transfer in-flight long enough to cancel.
    testModelFolder = createLargeTestModelFolder(120)
    const modelName = 'Test Cancel Mid Transfer'

    await openUploadDrawer(page)
    await byTestId(page, 'llm-param-display_name').fill(modelName)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await submitUploadForm(page)

    // The Upload Progress card's "Cancel Upload" link aborts mid-transfer.
    const cancelUpload = byTestId(page, 'llm-upload-cancel-btn')
    await expect(cancelUpload).toBeVisible({ timeout: 15000 })
    await cancelUpload.click()

    // No success toast, and the uploading state clears.
    await expect(page.locator('[data-sonner-toast][data-type="success"]')).toHaveCount(0)
    await expect(cancelUpload).toBeHidden({ timeout: 15000 })

    // Close the drawer and confirm the model never landed in the list.
    await cancelUploadForm(page)
    await assertModelNotExists(page, modelName)
  })

  test('should prevent drawer from closing during upload', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await byTestId(page, 'llm-param-display_name').fill('Test Upload Prevent Close')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await submitUploadForm(page)

    // The footer Cancel button is disabled while uploading (or the upload
    // already finished for the tiny fixture).
    await page.waitForTimeout(100)
    const cancelButton = byTestId(page, 'llm-upload-drawer-cancel-btn')
    const isButtonDisabled = await cancelButton.isDisabled().catch(() => false)
    const succeeded = await byTestId(page, 'llm-model-upload-form').isHidden().catch(() => false)
    expect(isButtonDisabled || succeeded).toBe(true)

    await expectUploadSucceeded(page)
    await deleteModel(page, 'Test Upload Prevent Close')
  })

  test('should show overall progress percentage', async ({ page }) => {
    testModelFolder = await createTestModelFolder('safetensors')

    await openUploadDrawer(page)
    await byTestId(page, 'llm-param-display_name').fill('Test Overall Progress')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await submitUploadForm(page)

    await expectUploadSucceeded(page)
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
    await byTestId(page, 'llm-param-display_name').fill(modelName)
    await byTestId(page, 'llm-param-description').fill('Test model upload description')
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)

    await submitUploadForm(page)

    // Drawer closes on success and the model appears in the list.
    await expectUploadSucceeded(page)
    await assertModelExists(page, modelName)

    await deleteModel(page, modelName)
  })

  test('should reset form after successful upload', async ({ page }) => {
    testModelFolder = await createTestModelFolder('gguf')
    const modelName = `test-model-reset-${Date.now()}`

    await openUploadDrawer(page)
    await byTestId(page, 'llm-param-display_name').fill(modelName)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)
    await submitUploadForm(page)

    await expectUploadSucceeded(page)
    // Let any success toast clear before reopening.
    await page.locator('[data-sonner-toast][data-type="success"]').first().waitFor({ state: 'hidden', timeout: 10000 }).catch(() => {})
    await page.waitForTimeout(500)

    // Open drawer again — form is reset.
    await openUploadDrawer(page)
    await expect(byTestId(page, 'llm-param-display_name')).toHaveValue('')
    await expect(byTestId(page, 'llm-upload-selected-files-card')).not.toBeVisible()

    await cancelUploadForm(page)
    await deleteModel(page, modelName)
  })

  test('should show uploaded model immediately in models list', async ({ page }) => {
    testModelFolder = await createTestModelFolder('pytorch')
    const modelName = `test-model-immediate-${Date.now()}`

    await openUploadDrawer(page)
    await byTestId(page, 'llm-param-display_name').fill(modelName)
    await uploadModelFolder(page, testModelFolder)
    await page.waitForTimeout(500)
    await submitUploadForm(page)

    await expectUploadSucceeded(page)
    await assertModelExists(page, modelName)

    await deleteModel(page, modelName)
  })
})
