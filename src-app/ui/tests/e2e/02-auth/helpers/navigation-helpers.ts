import { Page } from '@playwright/test'
import { clearAuthState } from '../../../common/auth-helpers'

/**
 * Auth-specific navigation helpers
 */

export async function goToAuthPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
  await page.waitForSelector('#login_username', { timeout: 30000 })
}

export async function logoutAndGoToAuth(page: Page, baseURL: string) {
  await clearAuthState(page)
  await goToAuthPage(page, baseURL)
}
