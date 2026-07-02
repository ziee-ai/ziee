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
 */

// ---- local helpers (testid-based) ----

async function openDownloadDrawer(page: Page) {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'download')
  await byTestId(page, 'llm-model-download-form').waitFor({ state: 'visible', timeout: 5000 })
}

async function selectRepo(page: Page, label: string) {
  await byTestId(page, 'llm-download-repository-select').click()
  await page
    .locator('[data-testid^="llm-download-repository-select-opt-"]')
    .filter({ hasText: label })
    .first()
    .click()
}

async function selectFormat(page: Page, value: string) {
  await byTestId(page, 'llm-file-format-select').click()
  await byTestId(page, `llm-file-format-select-opt-${value}`).click()
}

async function submitDownloadAndWait(page: Page) {
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/.*download/.test(r.url()) && r.request().method() === 'POST',
      { timeout: 15000 },
    ),
    byTestId(page, 'llm-download-drawer-submit-btn').click(),
  ])
  expect(resp.ok()).toBeTruthy()
}

async function cancelDownloadForm(page: Page) {
  // A kit Select popup opened earlier in the drawer can leave a closing
  // animation that keeps the footer button reporting "not stable"; dismiss any
  // open popup first, then force past the stability wait for this teardown click.
  await page.keyboard.press('Escape')
  await byTestId(page, 'llm-download-drawer-cancel-btn').click({ force: true })
  await byTestId(page, 'llm-model-download-form').waitFor({ state: 'hidden', timeout: 5000 })
}

async function closeViewDetails(page: Page) {
  const close = byTestId(page, 'llm-download-drawer-close-btn')
  await close.waitFor({ state: 'visible', timeout: 5000 })
  await close.click()
  await close.waitFor({ state: 'hidden', timeout: 5000 })
}

test.describe('LLM Models - Local Download - UI Structure', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-provider-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Download test provider')
    await clickProviderCard(page, testProvider)
  })

  test('should pass accessibility checks for download drawer', async ({ page }) => {
    await openAddModelDropdown(page)
    await selectAddModelOption(page, 'download')
    await assertNoAccessibilityViolations(page)
  })

  test('should display download drawer with correct structure', async ({ page }) => {
    await openDownloadDrawer(page)

    // Verify form fields are present.
    await expect(byTestId(page, 'llm-download-repository-select')).toBeVisible()
    await expect(byTestId(page, 'llm-download-repository-path-input')).toBeVisible()
    await expect(byTestId(page, 'llm-download-branch-input')).toBeVisible()
    await expect(byTestId(page, 'llm-param-display_name')).toBeVisible()
    await expect(byTestId(page, 'llm-download-main-filename-input')).toBeVisible()
    await expect(byTestId(page, 'llm-file-format-select')).toBeVisible()
    await expect(byTestId(page, 'llm-engine-type-select')).toBeVisible()

    // Verify buttons.
    await expect(byTestId(page, 'llm-download-drawer-cancel-btn')).toBeVisible()
    await expect(byTestId(page, 'llm-download-drawer-submit-btn')).toBeVisible()

    await cancelDownloadForm(page)
  })

  test('should show repository dropdown', async ({ page }) => {
    await openDownloadDrawer(page)

    await byTestId(page, 'llm-download-repository-select').click()
    // Verify Hugging Face Hub option exists (default repository).
    await expect(
      page
        .locator('[data-testid^="llm-download-repository-select-opt-"]')
        .filter({ hasText: 'Hugging Face Hub' }),
    ).toBeVisible()

    await page.keyboard.press('Escape')
    await cancelDownloadForm(page)
  })

  test('should show example repository paths', async ({ page }) => {
    await openDownloadDrawer(page)

    const repositoryPathInput = byTestId(page, 'llm-download-repository-path-input')
    await expect(repositoryPathInput).toBeVisible()
    const placeholder = await repositoryPathInput.getAttribute('placeholder')
    expect(placeholder).toBeTruthy()

    await cancelDownloadForm(page)
  })
})

test.describe('LLM Models - Local Download - Form Validation', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-validation-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Download validation test provider')
    await clickProviderCard(page, testProvider)
  })

  test('should validate required fields', async ({ page }) => {
    await openDownloadDrawer(page)

    // Try to submit without filling any fields → validation errors (role=alert).
    await byTestId(page, 'llm-download-drawer-submit-btn').click()
    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelDownloadForm(page)
  })

  test('should validate repository is required', async ({ page }) => {
    await openDownloadDrawer(page)

    // Fill other fields but not repository.
    await byTestId(page, 'llm-download-repository-path-input').fill('test/model')
    await byTestId(page, 'llm-param-display_name').fill('Test Model')
    await byTestId(page, 'llm-download-main-filename-input').fill('model.safetensors')

    await byTestId(page, 'llm-download-drawer-submit-btn').click()
    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelDownloadForm(page)
  })

  test('should validate repository path is required', async ({ page }) => {
    await openDownloadDrawer(page)

    await byTestId(page, 'llm-download-repository-path-input').fill('')
    await selectRepo(page, 'Hugging Face Hub')

    await byTestId(page, 'llm-download-drawer-submit-btn').click()
    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelDownloadForm(page)
  })

  test('should validate display name is required', async ({ page }) => {
    await openDownloadDrawer(page)

    await byTestId(page, 'llm-param-display_name').fill('')
    await selectRepo(page, 'Hugging Face Hub')

    await byTestId(page, 'llm-download-drawer-submit-btn').click()
    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelDownloadForm(page)
  })

  test('should validate main filename is required', async ({ page }) => {
    await openDownloadDrawer(page)

    await byTestId(page, 'llm-download-main-filename-input').fill('')
    await selectRepo(page, 'Hugging Face Hub')

    await byTestId(page, 'llm-download-drawer-submit-btn').click()
    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 })

    await cancelDownloadForm(page)
  })
})

test.describe('LLM Models - Local Download - Download Initiation', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-initiate-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Download initiation test provider')
    await clickProviderCard(page, testProvider)
  })

  test('should start download with valid data', async ({ page }) => {
    const modelName = `test-download-${Date.now()}`

    await openDownloadDrawer(page)

    await selectRepo(page, 'Hugging Face Hub')
    await byTestId(page, 'llm-download-repository-path-input').fill('hf-internal-testing/tiny-random-gpt2')
    await byTestId(page, 'llm-param-display_name').fill(modelName)
    await byTestId(page, 'llm-download-main-filename-input').fill('model.safetensors')
    await selectFormat(page, 'safetensors')

    await submitDownloadAndWait(page)

    // Form drawer closes; the View Download Details drawer auto-opens.
    await byTestId(page, 'llm-download-drawer-submit-btn').waitFor({ state: 'hidden', timeout: 5000 })
    await closeViewDetails(page)
  })

  test('should show error for non-existent repository path', async ({ page }) => {
    const invalidModelName = `invalid-model-${Date.now()}`

    await openDownloadDrawer(page)

    await selectRepo(page, 'Hugging Face Hub')
    await byTestId(page, 'llm-download-repository-path-input').fill('non-existent/invalid-model-path-12345')
    await byTestId(page, 'llm-param-display_name').fill(invalidModelName)
    await byTestId(page, 'llm-download-main-filename-input').fill('model.safetensors')
    await selectFormat(page, 'safetensors')

    // Download is accepted; validation happens asynchronously.
    await submitDownloadAndWait(page)
    await byTestId(page, 'llm-download-drawer-submit-btn').waitFor({ state: 'hidden', timeout: 5000 })

    // Download appears in the "Downloading Models" section with the model name.
    await expect(byTestId(page, 'llm-downloads-section-card')).toBeVisible({ timeout: 5000 })
    await expect(
      page.locator('[data-testid="llm-download-item-card"]').filter({ hasText: invalidModelName }),
    ).toBeVisible({ timeout: 5000 })

    // Backend worker processes asynchronously and eventually marks it failed.
    await expect(
      page.locator('[data-testid="llm-download-status-tag"]').filter({ hasText: 'Failed' }).first(),
    ).toBeVisible({ timeout: 40000 })

    // The failure surfaces in the auto-opened details drawer's progress card.
    await expect(byTestId(page, 'llm-download-progress-card')).toBeVisible()

    await closeViewDetails(page)
  })

  test('should handle optional branch parameter', async ({ page }) => {
    const modelName = `test-download-branch-${Date.now()}`

    await openDownloadDrawer(page)

    await selectRepo(page, 'Hugging Face Hub')
    await byTestId(page, 'llm-download-repository-path-input').fill('hf-internal-testing/tiny-random-gpt2')
    await byTestId(page, 'llm-download-branch-input').fill('main')
    await byTestId(page, 'llm-param-display_name').fill(modelName)
    await byTestId(page, 'llm-download-main-filename-input').fill('model.safetensors')
    await selectFormat(page, 'safetensors')

    await submitDownloadAndWait(page)
    await closeViewDetails(page)
  })
})

test.describe('LLM Models - Local Download - Progress Tracking', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-progress-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await configureHuggingFaceAuth(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Download progress test provider')
    await clickProviderCard(page, testProvider)
  })

  test('should show Downloads section when download is active', async ({ page }) => {
    const modelName = `test-download-progress-${Date.now()}`

    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    await expect(byTestId(page, 'llm-downloads-section-card')).toBeVisible({ timeout: 5000 })
    await closeViewDetails(page)
  })

  test('should show download progress for individual download', async ({ page }) => {
    const modelName = `test-download-individual-${Date.now()}`

    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    // Progress bar visible (details drawer auto-opened).
    await expect(byTestId(page, 'llm-download-detail-progress')).toBeVisible({ timeout: 10000 })
    await closeViewDetails(page)
  })

  test('should update download status in real-time via SSE', async ({ page }) => {
    const modelName = `test-download-sse-${Date.now()}`

    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    // SSE working → the status tag shows "Downloading...".
    await expect(
      page.locator('[data-testid="llm-download-status-tag"]').filter({ hasText: 'Downloading' }).first(),
    ).toBeVisible({ timeout: 10000 })
    await closeViewDetails(page)
  })

  test('should show model in models list after download completes', async ({ page }) => {
    const modelName = `test-download-complete-${Date.now()}`

    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    await closeViewDetails(page)

    // Give the small model time to download.
    await page.waitForTimeout(15000)

    await assertModelExists(page, modelName)
    await deleteModel(page, modelName)
  })
})

test.describe('LLM Models - Local Download - Download Management', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-management-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await configureHuggingFaceAuth(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Download management test provider')
    await clickProviderCard(page, testProvider)
  })

  test('should show cancel button for active downloads', async ({ page }) => {
    const modelName = `test-download-cancel-${Date.now()}`

    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-bert',
      mainFilename: 'model.safetensors',
    })

    // The details drawer's Cancel Download button is visible for active downloads.
    await expect(byTestId(page, 'llm-download-drawer-cancel-download-btn')).toBeVisible({ timeout: 10000 })
    await closeViewDetails(page)
  })

  test('should cancel an active download and remove it from the list', async ({ page }) => {
    // Mock the downloads LIST so a deterministic "downloading" row renders for
    // this provider with no real download, then exercise the real Cancel
    // wiring (POST /cancel + the store's immediate row removal).
    const providerId = new URL(page.url()).pathname.split('/').pop()!
    const downloadId = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'
    const now = new Date().toISOString()
    const downloadingRow = {
      id: downloadId,
      provider_id: providerId,
      repository_id: 'huggingface',
      status: 'downloading',
      created_at: now,
      started_at: now,
      updated_at: now,
      request_data: { model_name: 'cancel-me', display_name: 'cancel-me-download' },
      progress_data: {
        current: 1_000_000,
        total: 350_000_000,
        phase: 'downloading',
        message: '',
        speed_bps: 500_000,
        eta_seconds: 600,
      },
    }

    let cancelCalled = false

    await page.route(/\/api\/llm-models\/downloads\?/, async route => {
      if (route.request().method() !== 'GET') return route.fallback()
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          downloads: cancelCalled ? [] : [downloadingRow],
          page: 1,
          per_page: 100,
          total: cancelCalled ? 0 : 1,
        }),
      })
    })

    await page.route(`**/api/llm-models/downloads/${downloadId}/cancel`, async route => {
      cancelCalled = true
      await route.fulfill({ status: 204, body: '' })
    })

    await page.reload()

    // The downloading row + its Cancel button render in the section card.
    const card = byTestId(page, 'llm-downloads-section-card')
    const row = card.locator('[data-testid="llm-download-item-card"]').filter({ hasText: 'cancel-me-download' })
    await expect(row).toBeVisible({ timeout: 10000 })
    const cancelButton = row.locator('[data-testid="llm-download-compact-cancel-btn"], [data-testid="llm-download-cancel-btn"]').first()
    await expect(cancelButton).toBeVisible()

    const cancelRequest = page.waitForRequest(
      req =>
        req.url().includes(`/api/llm-models/downloads/${downloadId}/cancel`) &&
        req.method() === 'POST',
    )
    await cancelButton.click()
    await cancelRequest
    await expect(
      card.locator('[data-testid="llm-download-item-card"]').filter({ hasText: 'cancel-me-download' }),
    ).toHaveCount(0, { timeout: 10000 })
  })

  test('should remove download from list after completion', async ({ page }) => {
    const modelName = `test-download-remove-${Date.now()}`

    await startModelDownload(page, {
      displayName: modelName,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    await closeViewDetails(page)
    await page.waitForTimeout(15000)

    await assertModelExists(page, modelName)
    await deleteModel(page, modelName)
  })
})

test.describe('LLM Models - Local Download - Multiple Downloads', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-multiple-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await configureHuggingFaceAuth(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Multiple downloads test provider')
    await clickProviderCard(page, testProvider)
  })

  test('should handle multiple simultaneous downloads', async ({ page }) => {
    const model1Name = `test-download-multi-1-${Date.now()}`
    const model2Name = `test-download-multi-2-${Date.now()}`

    await startModelDownload(page, {
      displayName: model1Name,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'distilgpt2',
      mainFilename: 'model.safetensors',
      clearCache: true,
    })
    await closeViewDetails(page)
    await page.waitForTimeout(1000)

    await startModelDownload(page, {
      displayName: model2Name,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'openai-community/gpt2',
      mainFilename: 'model.safetensors',
      clearCache: true,
    })
    await closeViewDetails(page)

    await page.waitForTimeout(15000)

    await assertModelExists(page, model1Name)
    await assertModelExists(page, model2Name)

    await deleteModel(page, model1Name)
    await deleteModel(page, model2Name)
  })

  test('should show all active downloads in Downloads section', async ({ page }) => {
    const model1Name = `test-download-list-1-${Date.now()}`
    const model2Name = `test-download-list-2-${Date.now()}`

    await startModelDownload(page, {
      displayName: model1Name,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'distilgpt2',
      mainFilename: 'model.safetensors',
    })
    await closeViewDetails(page)

    await page.waitForTimeout(500)
    await startModelDownload(page, {
      displayName: model2Name,
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      repositoryId: 'huggingface',
      repositoryPath: 'openai-community/gpt2',
      mainFilename: 'model.safetensors',
    })
    await closeViewDetails(page)

    // Both downloads listed in the Downloading Models section.
    const section = byTestId(page, 'llm-downloads-section-card')
    await expect(section).toBeVisible({ timeout: 5000 })
    await expect(
      section.locator('[data-testid="llm-download-item-card"]').filter({ hasText: model1Name }),
    ).toBeVisible({ timeout: 5000 })
    await expect(
      section.locator('[data-testid="llm-download-item-card"]').filter({ hasText: model2Name }),
    ).toBeVisible({ timeout: 5000 })
  })
})

test.describe('LLM Models - Local Download - Auto-detect files', () => {
  let testProvider: string

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    testProvider = `test-download-detect-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await configureHuggingFaceAuth(page, baseURL)
    await createLocalProvider(page, baseURL, testProvider, 'Detect files test provider')
    await clickProviderCard(page, testProvider)
  })

  test('Detect files auto-fills a GGUF quant from the live API', async ({ page }) => {
    await openDownloadDrawer(page)

    await selectRepo(page, 'Hugging Face Hub')
    await byTestId(page, 'llm-download-repository-path-input').fill('Qwen/Qwen2.5-0.5B-Instruct-GGUF')

    await byTestId(page, 'llm-download-detect-files-btn').click()

    // The main filename is auto-filled with a detected .gguf quant — proof that
    // GGUF detection populated the picker.
    await expect(byTestId(page, 'llm-download-main-filename-input')).toHaveValue(/\.gguf$/i, {
      timeout: 30000,
    })

    await cancelDownloadForm(page)
  })

  test('Detect files auto-selects the whole safetensors set', async ({ page }) => {
    await openDownloadDrawer(page)

    await selectRepo(page, 'Hugging Face Hub')
    await byTestId(page, 'llm-download-repository-path-input').fill('hf-internal-testing/tiny-random-gpt2')

    await byTestId(page, 'llm-download-detect-files-btn').click()

    await expect(byTestId(page, 'llm-download-main-filename-input')).toHaveValue(/safetensors/i, {
      timeout: 30000,
    })

    // The help text tells the user shards are pulled automatically.
    await expect(
      page.locator('[data-testid="llm-model-download-form"]').locator('text=/full weight set downloads automatically/i'),
    ).toBeVisible()

    await cancelDownloadForm(page)
  })

  // audit id ad0a9459e69e — these cover the failure branches of handleDetectFiles.

  test('Detect files with no repository path shows the validation error', async ({ page }) => {
    await openDownloadDrawer(page)

    await selectRepo(page, 'Hugging Face Hub')
    // Wait for the selection to register (and the Select popup to close) so the
    // Detect click lands immediately instead of racing the closing popup.
    await expect(byTestId(page, 'llm-download-repository-select')).toContainText('Hugging Face Hub')

    // Leave the repository path EMPTY and click Detect.
    await byTestId(page, 'llm-download-detect-files-btn').click()

    await expect(
      page.locator('[data-sonner-toast]').filter({ hasText: /Select a repository and enter a repository path first/i }),
    ).toBeVisible({ timeout: 5000 })

    await cancelDownloadForm(page)
  })

  test('Detect files surfaces a backend error as a toast', async ({ page }) => {
    await page.route(/\/api\/llm-models\/repository-files(\?|$)/, async route => {
      await route.fulfill({
        status: 502,
        contentType: 'application/json',
        body: JSON.stringify({
          error_code: 'UPSTREAM_ERROR',
          error: 'repository host unavailable',
        }),
      })
    })

    await openDownloadDrawer(page)

    await selectRepo(page, 'Hugging Face Hub')
    await byTestId(page, 'llm-download-repository-path-input').fill('Qwen/whatever')

    await byTestId(page, 'llm-download-detect-files-btn').click()

    await expect(
      page.locator('[data-sonner-toast]').filter({ hasText: /Failed to detect files:/i }),
    ).toBeVisible({ timeout: 10000 })

    await cancelDownloadForm(page)
  })

  test('Detect files with an empty file set warns "No files found"', async ({ page }) => {
    await page.route(/\/api\/llm-models\/repository-files(\?|$)/, async route => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          files: [],
          shape: 'gguf',
          source: 'huggingface',
          truncated: false,
        }),
      })
    })

    await openDownloadDrawer(page)

    await selectRepo(page, 'Hugging Face Hub')
    await byTestId(page, 'llm-download-repository-path-input').fill('Qwen/empty-repo')

    await byTestId(page, 'llm-download-detect-files-btn').click()

    await expect(
      page.locator('[data-sonner-toast]').filter({ hasText: /No files found for that repository path \/ branch\./i }),
    ).toBeVisible({ timeout: 10000 })

    await cancelDownloadForm(page)
  })
})
