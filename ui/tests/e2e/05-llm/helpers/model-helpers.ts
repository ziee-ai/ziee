import { Page, expect } from '@playwright/test'
import { fillModelCommonFields, fillModelCapabilities, fillModelParameters, fillModelEngineSettings, fillDownloadForm, fillUploadForm, submitUploadForm, type ModelFormData, type DownloadFormData, type UploadFormData } from './form-helpers'

/**
 * LLM Model CRUD helpers
 */

// =====================================================
// Model Upload
// =====================================================

export async function openAddModelDropdown(page: Page) {
  // Click the + button in the Models card header
  // This button opens a dropdown with "Upload Model", "Download from Repository", etc.
  const addButton = page.locator('.ant-card-head:has-text("Models") button[data-icon="plus"], .ant-card-head:has-text("Models") button:has([data-icon="plus"])')
  await addButton.click()
  await page.waitForSelector('.ant-dropdown-menu', { timeout: 5000 })
}

export async function selectAddModelOption(page: Page, option: 'upload' | 'download' | 'remote') {
  const optionMap = {
    upload: 'Upload from Files',
    download: 'Download from Repository',
    remote: 'Add Remote Model',
  }
  await page.click(`text=${optionMap[option]}`)
  await page.waitForLoadState('networkidle')
}

export async function uploadModelFolder(page: Page, folderPath: string) {
  // For folder uploads, we need to use the directory input
  const fileInput = page.locator('input[type="file"][webkitdirectory]')
  await fileInput.setInputFiles(folderPath)
  await page.waitForLoadState('networkidle')
}

export async function uploadModelFile(page: Page, filePath: string) {
  const fileInput = page.locator('input[type="file"]')
  await fileInput.setInputFiles(filePath)
  await page.waitForLoadState('networkidle')
}

export async function uploadModel(
  page: Page,
  data: UploadFormData
): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'upload')

  // Wait for upload drawer to open
  await page.waitForSelector('.ant-drawer-title:has-text("Upload Local Model")', { timeout: 5000 })

  // Upload folder
  await uploadModelFolder(page, data.folderPath)

  // Fill form
  await fillUploadForm(page, data)

  // Submit
  await submitUploadForm(page)

  // Wait for success message
  await page.waitForSelector('text=Model uploaded successfully', { timeout: 30000 })
}

export async function waitForUploadProgress(page: Page): Promise<void> {
  // Wait for upload progress card to appear
  await page.waitForSelector('.ant-card-head-title:has-text("Upload Progress")', { timeout: 5000 })

  // Wait for progress to complete (progress bar reaches 100% or upload completes)
  await page.waitForSelector('.ant-progress-status-success', { timeout: 60000 })
}

export async function assertUploadProgressVisible(page: Page): Promise<void> {
  await expect(page.locator('.ant-card-head-title:has-text("Upload Progress")')).toBeVisible()
  await expect(page.locator('.ant-progress-line')).toBeVisible()
}

export async function openUploadDrawer(page: Page): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'upload')
  await page.waitForSelector('.ant-drawer-title:has-text("Upload Local Model")', { timeout: 5000 })
}

// =====================================================
// Model Download
// =====================================================

export async function startModelDownload(
  page: Page,
  data: DownloadFormData
): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'download')

  await fillDownloadForm(page, data)
  await page.click('button:has-text("Start Download")')

  // Wait for success message
  await page.waitForSelector('text=Download started successfully', { timeout: 15000 })
}

// =====================================================
// Model Editing
// =====================================================

export async function openEditModelDrawer(page: Page, modelName: string) {
  const modelCard = page.locator(`text=${modelName}`).first()
  const editButton = modelCard.locator('button[aria-label="Edit"]')
  await editButton.click()
  await page.waitForSelector('text=Edit Model', { timeout: 30000 })
}

// =====================================================
// Model Deletion
// =====================================================

export async function deleteModel(page: Page, modelName: string): Promise<void> {
  const modelCard = page.locator(`text=${modelName}`).first()
  const deleteButton = modelCard.locator('button[aria-label="Delete"]')
  await deleteButton.click()

  // Confirm deletion
  await page.waitForSelector('text=Delete Model', { timeout: 5000 })
  await page.click('button:has-text("Delete")')
  await page.waitForSelector('text=Model deleted successfully', { timeout: 15000 })
}

// =====================================================
// Model Assertions
// =====================================================

export async function assertModelExists(page: Page, modelName: string): Promise<void> {
  await expect(page.locator(`text=${modelName}`).first()).toBeVisible()
}

export async function assertModelNotExists(page: Page, modelName: string): Promise<void> {
  await expect(page.locator(`text=${modelName}`).first()).not.toBeVisible()
}
