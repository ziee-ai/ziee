import { Page, expect } from '@playwright/test'
import { fillModelCommonFields, fillModelCapabilities, fillModelParameters, fillModelEngineSettings, fillDownloadForm, fillUploadForm, submitUploadForm, type ModelFormData, type DownloadFormData, type UploadFormData } from './form-helpers'

/**
 * LLM Model CRUD helpers
 */

// =====================================================
// UI Cleanup Utilities
// =====================================================

/**
 * Close all open drawers to ensure clean UI state before interactions
 * This prevents drawer elements from intercepting pointer events
 *
 * This helper is deterministic and safe to call even when no drawers are open
 */
export async function closeAnyOpenDrawers(page: Page): Promise<void> {
  // Attempt to close all visible drawers by clicking close buttons
  const closeButtons = page.locator('.ant-drawer:visible button[aria-label="Close drawer"]')
  const count = await closeButtons.count()

  // Click each close button sequentially
  for (let i = 0; i < count; i++) {
    await closeButtons.nth(0).click({ timeout: 2000 }).catch(() => {
      // Button may disappear as drawer closes - this is expected
    })
    await page.waitForTimeout(300) // Wait for close animation
  }

  // Press Escape as additional cleanup (safe even if no drawers)
  await page.keyboard.press('Escape')
  await page.waitForTimeout(300)
}

// =====================================================
// Model Upload
// =====================================================

export async function openAddModelDropdown(page: Page) {
  // Close any open drawers first to prevent pointer interception
  await closeAnyOpenDrawers(page)

  // Click the + button in the Models card header
  // This button opens a dropdown with "Upload Model", "Download from Repository", etc.
  const addButton = page.locator('.ant-card-head:has-text("Models") button[data-icon="plus"], .ant-card-head:has-text("Models") button:has([data-icon="plus"])')

  // Wait for button to be ready and visible
  await addButton.waitFor({ state: 'visible', timeout: 10000 })

  // Ensure button is enabled and stable before clicking
  await expect(addButton).toBeEnabled()
  await page.waitForTimeout(300) // Small delay to ensure button is fully interactive

  await addButton.click()

  // Wait for dropdown menu to appear and be stable
  const dropdown = page.locator('.ant-dropdown-menu')
  await dropdown.waitFor({ state: 'visible', timeout: 10000 })
  await expect(dropdown).toBeVisible()
}

export async function selectAddModelOption(page: Page, option: 'upload' | 'download' | 'remote') {
  const optionMap = {
    upload: 'Upload from Files',
    download: 'Download from Repository',
    remote: 'Add Remote Model',
  }

  // Scope selector to dropdown menu to avoid strict mode violations with drawer headings
  const menuItem = page.locator('.ant-dropdown-menu').locator(`text=${optionMap[option]}`)
  await menuItem.waitFor({ state: 'visible', timeout: 10000 })

  await menuItem.click()

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
  await page.waitForSelector('.ant-card-head-title:has-text("Selected Files")', { timeout: 10000 })

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

  // Wait for drawer title to appear
  const drawerTitle = page.locator('.ant-drawer-title:has-text("Upload Local Model")')
  await drawerTitle.waitFor({ state: 'visible', timeout: 10000 })

  // Scope to the upload drawer to avoid conflicts with other drawers
  const uploadDrawer = page.locator('.ant-drawer:visible:has(.ant-drawer-title:has-text("Upload Local Model"))')

  // Wait for drawer to be fully loaded - check that form fields and buttons are present
  await expect(uploadDrawer.locator('label:has-text("Display Name")')).toBeVisible()
  await expect(uploadDrawer.locator('button:has-text("Upload")')).toBeVisible()
  await expect(uploadDrawer.locator('button:has-text("Cancel")')).toBeVisible()

  // Ensure buttons are enabled (not in uploading state)
  await expect(uploadDrawer.locator('button:has-text("Upload")')).toBeEnabled({ timeout: 5000 })
  await expect(uploadDrawer.locator('button:has-text("Cancel")')).toBeEnabled({ timeout: 5000 })

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
  await page.waitForSelector('.ant-drawer-title:has-text("Download from Repository")', { timeout: 10000 })
  await page.waitForTimeout(500) // Allow drawer to fully render

  await fillDownloadForm(page, data)

  const downloadButton = page.locator('button:has-text("Start Download")')
  await expect(downloadButton).toBeEnabled()
  await downloadButton.click()

  // Wait for success message
  await page.waitForSelector('text=Download started successfully', { timeout: 15000 })

  // Wait for drawer to close completely
  await page.waitForSelector('.ant-drawer-title:has-text("Download from Repository")', { state: 'hidden', timeout: 10000 })
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
  // Close any open drawers first to prevent pointer interception
  await closeAnyOpenDrawers(page)

  // Find the delete button by its aria-label which includes the model display name
  const deleteButton = page.locator(`button[aria-label*="Delete"][aria-label*="${modelName}"]`)
  await deleteButton.click()

  // Wait for success message (no confirmation modal)
  await page.waitForSelector('text=Model deleted', { timeout: 15000 })
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
