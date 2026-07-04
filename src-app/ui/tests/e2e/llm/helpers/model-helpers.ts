import { Page, expect } from '@playwright/test'
import { fillDownloadForm, fillUploadForm, submitUploadForm, type DownloadFormData, type UploadFormData } from './form-helpers'
import { byTestId } from '../../testid'
import * as fs from 'fs'
import * as path from 'path'

/**
 * LLM Model CRUD helpers (kit / data-testid based)
 *
 * IMPORTANT: These helpers should NOT handle drawer cleanup.
 * Each test is responsible for explicitly closing any drawers it opens.
 */

// =====================================================
// Model Upload / Add
// =====================================================

export async function openAddModelDropdown(page: Page) {
  // The Models card "Add" button. For local providers this opens a kit
  // Dropdown (upload / download); for remote it opens the remote drawer.
  const localBtn = byTestId(page, 'llm-models-add-local-btn')
  const remoteBtn = byTestId(page, 'llm-models-add-remote-btn')
  const trigger = (await localBtn.count()) ? localBtn : remoteBtn
  await trigger.first().waitFor({ state: 'visible', timeout: 10000 })
  await expect(trigger.first()).toBeEnabled()
  await page.waitForTimeout(300)
  await trigger.first().click()
}

export async function selectAddModelOption(page: Page, option: 'upload' | 'download' | 'remote') {
  if (option === 'remote') {
    // Remote provider: the add button itself opens the drawer (no menu).
    await byTestId(page, 'llm-model-upload-form')
      .or(byTestId(page, 'llm-model-download-form'))
      .first()
      .waitFor({ state: 'visible', timeout: 10000 })
      .catch(() => {})
    return
  }
  const item = byTestId(page, `llm-models-add-dropdown-item-${option}`)
  await item.waitFor({ state: 'visible', timeout: 10000 })
  await item.click()
  await page.waitForLoadState('domcontentloaded')
  await page.waitForTimeout(500)
}

export async function uploadModelFolder(page: Page, folderPath: string) {
  // The kit Upload puts its testid on a display:contents wrapper; the real <input
  // type="file"> is a descendant (a sibling of the role=button dropzone, for a11y
  // — not nested inside it). No webkitdirectory — it's a multiple input.
  const fileInput = byTestId(page, 'llm-upload-files').locator('input[type="file"]')
  // The input is a plain `multiple` file input (no webkitdirectory), so Playwright
  // cannot accept a directory path — enumerate the folder and pass each file.
  const files = fs
    .readdirSync(folderPath)
    .map((name) => path.join(folderPath, name))
    .filter((p) => fs.statSync(p).isFile())
  await fileInput.setInputFiles(files)
  await page.waitForLoadState('domcontentloaded')
  // Wait for the selected-files list (indicates files were processed).
  await byTestId(page, 'llm-upload-selected-files-list').waitFor({ timeout: 10000 })
  await page.waitForTimeout(500)
}

export async function uploadModelFile(page: Page, filePath: string) {
  const fileInput = page.locator('input[type="file"]')
  await fileInput.setInputFiles(filePath)
  await page.waitForLoadState('load')
}

export async function uploadModel(page: Page, data: UploadFormData): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'upload')

  // Wait for the upload form to render.
  await byTestId(page, 'llm-model-upload-form').waitFor({ timeout: 5000 })

  await uploadModelFolder(page, data.folderPath)
  await fillUploadForm(page, data)

  // Submit and prove the server accepted the upload.
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/.*models/.test(r.url()) && r.request().method() === 'POST',
      { timeout: 30000 }
    ),
    submitUploadForm(page),
  ])
  expect(resp.ok()).toBeTruthy()
}

export async function waitForUploadProgress(page: Page): Promise<void> {
  await byTestId(page, 'llm-upload-progress-card').waitFor({ timeout: 5000 })
  await byTestId(page, 'llm-upload-overall-progress').waitFor({ timeout: 60000 })
}

export async function assertUploadProgressVisible(page: Page): Promise<void> {
  await expect(byTestId(page, 'llm-upload-progress-card')).toBeVisible()
  await expect(byTestId(page, 'llm-upload-overall-progress')).toBeVisible()
}

export async function openUploadDrawer(page: Page): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'upload')

  const form = byTestId(page, 'llm-model-upload-form')
  await form.waitFor({ state: 'visible', timeout: 10000 })

  // The form fields + footer buttons must be present and interactive.
  await expect(byTestId(page, 'llm-param-display_name')).toBeVisible()
  const uploadButton = byTestId(page, 'llm-upload-drawer-submit-btn')
  const cancelButton = byTestId(page, 'llm-upload-drawer-cancel-btn')
  await expect(uploadButton).toBeVisible()
  await expect(cancelButton).toBeVisible()
  await expect(cancelButton).toBeEnabled({ timeout: 5000 })
  await page.waitForTimeout(300)
}

// =====================================================
// Model Download
// =====================================================

export async function startModelDownload(page: Page, data: DownloadFormData): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'download')

  const form = byTestId(page, 'llm-model-download-form')
  await form.waitFor({ timeout: 10000 })
  await page.waitForTimeout(500)

  await fillDownloadForm(page, data)

  const downloadButton = byTestId(page, 'llm-download-drawer-submit-btn')
  await expect(downloadButton).toBeEnabled()
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/.*download/.test(r.url()) && r.request().method() === 'POST',
      { timeout: 15000 }
    ),
    downloadButton.click(),
  ])
  expect(resp.ok()).toBeTruthy()

  // On a successful start the shared drawer switches from the editable
  // add-form to the read-only "View Download Details" mode (they share one
  // Drawer, so `llm-model-download-form` stays mounted). The add-mode submit
  // button disappearing is the reliable signal that the transition happened.
  void form
  await byTestId(page, 'llm-download-drawer-submit-btn').waitFor({ state: 'hidden', timeout: 10000 })
}

// =====================================================
// Model Editing
// =====================================================

export async function openEditModelDrawer(page: Page, modelName: string) {
  // Edit buttons carry `aria-label="Edit ${displayName} model"`.
  await page.locator(`[aria-label="Edit ${modelName} model"]`).first().click()
  await byTestId(page, 'llm-edit-model-form').waitFor({ timeout: 30000 })
}

// =====================================================
// Model Deletion
// =====================================================

export async function deleteModel(page: Page, modelName: string): Promise<void> {
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/.*models/.test(r.url()) && r.request().method() === 'DELETE',
      { timeout: 15000 }
    ),
    page.locator(`[aria-label="Delete ${modelName} model"]`).first().click(),
  ])
  expect(resp.ok()).toBeTruthy()
}

// =====================================================
// Model Assertions
// =====================================================

export async function assertModelExists(page: Page, modelName: string): Promise<void> {
  // Each model row exposes an edit button labelled with its display name.
  await expect(page.locator(`[aria-label="Edit ${modelName} model"]`).first()).toBeVisible()
}

export async function assertModelNotExists(page: Page, modelName: string): Promise<void> {
  await expect(page.locator(`[aria-label="Edit ${modelName} model"]`).first()).not.toBeVisible()
}
