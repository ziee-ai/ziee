import { Page, expect } from '@playwright/test'
import { fillDownloadForm, fillUploadForm, submitUploadForm, type DownloadFormData, type UploadFormData } from './form-helpers'

/**
 * LLM Model CRUD helpers
 *
 * IMPORTANT: These helpers should NOT handle drawer cleanup.
 * Each test is responsible for explicitly closing any drawers it opens.
 */

// =====================================================
// Model Upload
// =====================================================

export async function openAddModelDropdown(page: Page) {
  // Click the + button in the Models card header
  // Use semantic selector with fallback
  const addButton = page.getByRole('button', { name: /add.*model/i })
    .or(page.locator('.ant-card-head:has-text("Models") button[data-icon="plus"], .ant-card-head:has-text("Models") button:has([data-icon="plus"])'))

  // Wait for button to be ready and visible
  await addButton.waitFor({ state: 'visible', timeout: 10000 })

  // Ensure button is enabled and stable before clicking
  await expect(addButton.first()).toBeEnabled()
  await page.waitForTimeout(300) // Small delay to ensure button is fully interactive

  await addButton.first().click()

  // Wait for dropdown menu to appear and be stable
  await page.getByRole('menu').or(page.locator('.ant-dropdown-menu')).waitFor({ state: 'visible', timeout: 10000 })
}

export async function selectAddModelOption(page: Page, option: 'upload' | 'download' | 'remote') {
  const optionMap = {
    upload: 'Upload from Files',
    download: 'Download from Repository',
    remote: 'Add Remote Model',
  }

  // Use semantic selector with fallback to text
  const menuItem = page.getByRole('menuitem', { name: optionMap[option] })
    .or(page.locator('.ant-dropdown-menu').getByText(optionMap[option]))

  await menuItem.waitFor({ state: 'visible', timeout: 10000 })
  await menuItem.first().click()

  // Wait for network to settle after clicking
  await page.waitForLoadState('domcontentloaded')
  await page.waitForTimeout(500) // Additional delay for drawer animation
}

export async function uploadModelFolder(page: Page, folderPath: string) {
  // For folder uploads, we need to use the directory input
  const fileInput = page.locator('input[type="file"][webkitdirectory]')
  await fileInput.setInputFiles(folderPath)

  // Wait for file processing to complete
  await page.waitForLoadState('domcontentloaded')

  // Wait for the file list to appear (indicates files were processed)
  await page.getByText('Selected Files').or(page.locator('.ant-card-head-title:has-text("Selected Files")')).waitFor({ timeout: 10000 })

  // Additional delay to ensure file classification completes
  await page.waitForTimeout(500)
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
  await page.getByRole('dialog', { name: /upload.*model/i })
    .or(page.locator('.ant-drawer-title:has-text("Upload Local Model")'))
    .waitFor({ timeout: 5000 })

  // Upload folder
  await uploadModelFolder(page, data.folderPath)

  // Fill form
  await fillUploadForm(page, data)

  // Submit
  await submitUploadForm(page)

  // Wait for success message
  await page.getByText('Model uploaded successfully').waitFor({ timeout: 30000 })
}

export async function waitForUploadProgress(page: Page): Promise<void> {
  // Wait for upload progress card to appear
  await page.getByText('Upload Progress').or(page.locator('.ant-card-head-title:has-text("Upload Progress")')).waitFor({ timeout: 5000 })

  // Wait for progress to complete (progress bar reaches 100% or upload completes)
  await page.locator('.ant-progress-status-success').waitFor({ timeout: 60000 })
}

export async function assertUploadProgressVisible(page: Page): Promise<void> {
  await expect(page.getByText('Upload Progress').or(page.locator('.ant-card-head-title:has-text("Upload Progress")'))).toBeVisible()
  await expect(page.locator('.ant-progress-line')).toBeVisible()
}

export async function openUploadDrawer(page: Page): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'upload')

  // Wait for drawer to appear using semantic selector
  const uploadDrawer = page.getByRole('dialog', { name: /upload.*model/i })
    .or(page.locator('.ant-drawer:visible:has(.ant-drawer-title:has-text("Upload Local Model"))'))

  await uploadDrawer.waitFor({ state: 'visible', timeout: 10000 })

  // Wait for drawer to be fully loaded - check that form fields and buttons are present
  await expect(uploadDrawer.getByLabel(/display name/i).or(uploadDrawer.locator('label:has-text("Display Name")'))).toBeVisible()
  await expect(uploadDrawer.getByRole('button', { name: 'Upload' })).toBeVisible()
  await expect(uploadDrawer.getByRole('button', { name: 'Cancel' })).toBeVisible()

  // Ensure buttons are enabled (not in uploading state)
  await expect(uploadDrawer.getByRole('button', { name: 'Upload' })).toBeEnabled({ timeout: 5000 })
  await expect(uploadDrawer.getByRole('button', { name: 'Cancel' })).toBeEnabled({ timeout: 5000 })

  // Small delay to ensure all animations are complete
  await page.waitForTimeout(300)
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

  // Wait for drawer to be fully loaded
  await page.getByRole('dialog', { name: /download.*repository/i })
    .or(page.locator('.ant-drawer-title:has-text("Download from Repository")'))
    .waitFor({ timeout: 10000 })
  await page.waitForTimeout(500) // Allow drawer to fully render

  await fillDownloadForm(page, data)

  const downloadButton = page.getByRole('button', { name: /start download/i })
  await expect(downloadButton).toBeEnabled()
  await downloadButton.click()

  // Wait for success message
  await page.getByText('Download started successfully').waitFor({ timeout: 15000 })

  // Wait for drawer to close completely
  await page.getByRole('dialog', { name: /download.*repository/i })
    .or(page.locator('.ant-drawer-title:has-text("Download from Repository")'))
    .waitFor({ state: 'hidden', timeout: 10000 })
}

// =====================================================
// Model Editing
// =====================================================

export async function openEditModelDrawer(page: Page, modelName: string) {
  // Use semantic selector for the edit button
  const editButton = page.getByRole('button', { name: new RegExp(`edit.*${modelName}`, 'i') })
    .or(page.locator(`text=${modelName}`).first().locator('button[aria-label="Edit"]'))

  await editButton.first().click()

  // Wait for edit drawer
  await page.getByRole('dialog', { name: /edit model/i })
    .or(page.getByText('Edit Model'))
    .waitFor({ timeout: 30000 })
}

// =====================================================
// Model Deletion
// =====================================================

export async function deleteModel(page: Page, modelName: string): Promise<void> {
  // Find the delete button by its aria-label which includes the model display name
  // Use semantic selector first, fallback to aria-label
  const deleteButton = page.getByRole('button', { name: new RegExp(`delete.*${modelName}`, 'i') })
    .or(page.locator(`button[aria-label*="Delete"][aria-label*="${modelName}"]`))

  await deleteButton.first().click()

  // Wait for success message (no confirmation modal)
  await page.getByText('Model deleted').waitFor({ timeout: 15000 })
}

// =====================================================
// Model Assertions
// =====================================================

export async function assertModelExists(page: Page, modelName: string): Promise<void> {
  await expect(page.getByText(modelName).first()).toBeVisible()
}

export async function assertModelNotExists(page: Page, modelName: string): Promise<void> {
  await expect(page.getByText(modelName).first()).not.toBeVisible()
}
