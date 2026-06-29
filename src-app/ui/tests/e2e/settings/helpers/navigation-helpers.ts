import { Page } from '@playwright/test'
import { byTestId } from '../../testid.ts'

/**
 * Settings-specific navigation helpers
 */

export type SettingsPage = 'general' | 'hardware' | 'llm-providers' | 'llm-repositories'

export async function goToSettingsPage(
  page: Page,
  baseURL: string,
  settingsPath: SettingsPage
) {
  await page.goto(`${baseURL}/settings/${settingsPath}`)
  await page.waitForLoadState('load')
}

/**
 * Wait until a settings section has rendered. The section title is i18n-safe
 * via the `settings-page-title` testid on SettingsPageContainer (the section
 * name itself is chrome, so we no longer assert it by text). The
 * `_expectedText` parameter is kept for call-site readability.
 */
export async function waitForSettingsPageLoad(page: Page, _expectedText?: string) {
  await byTestId(page, 'settings-page-title').waitFor({
    state: 'visible',
    timeout: 30000,
  })
}

/**
 * Drive the real Appearance theme <Select> (kit Select, testid
 * `settingsgen-theme-select`) on /settings/general. Options derive
 * `settingsgen-theme-select-opt-<value>`. i18n-safe — selects by value, not
 * by the visible label.
 */
export async function selectThemeOption(
  page: Page,
  value: 'light' | 'dark' | 'system'
) {
  await byTestId(page, 'settingsgen-theme-select').first().click()
  const option = byTestId(page, `settingsgen-theme-select-opt-${value}`)
  await option.waitFor({ state: 'visible', timeout: 5000 })
  await option.click()
}
