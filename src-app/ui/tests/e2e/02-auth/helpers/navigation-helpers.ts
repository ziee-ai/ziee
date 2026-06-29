import { Page } from '@playwright/test'
import { clearAuthState } from '../../../common/auth-helpers'
import { byTestId } from '../../testid'

/**
 * Auth-specific navigation helpers
 *
 * Uses testid selectors (i18n-safe) — see tests/e2e/testid.ts
 */

export async function goToAuthPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/auth`, { waitUntil: 'load' })
  await byTestId(page, 'auth-login-username').waitFor({ timeout: 30000 })
}

export async function logoutAndGoToAuth(page: Page, baseURL: string) {
  await clearAuthState(page)
  await goToAuthPage(page, baseURL)
}
