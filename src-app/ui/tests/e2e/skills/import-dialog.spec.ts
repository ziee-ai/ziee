import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'
import { byTestId } from '../testid.ts'

// E2E for ImportSkillDialog (audit id e4d562a5229ca438 — user-scope skill
// import had zero coverage): the file picker, the Validate button + inline
// validation-result rendering (valid + invalid), and the non-md advisory
// branch. Only the server-side /api/skills/validate boundary is mocked.

type ValEntry = { code: string; location?: string; message: string }
type ValResponse = { valid: boolean; errors: ValEntry[]; warnings: ValEntry[] }

async function mockValidate(page: Page, resp: ValResponse) {
  await page.route(/\/api\/skills\/validate$/, async route => {
    if (route.request().method() === 'POST') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(resp),
      })
    }
    return route.continue()
  })
}

async function openImportDialog(page: Page) {
  await byTestId(page, 'skill-list-import-button').click()
  await expect(byTestId(page, 'skill-import-dialog')).toBeVisible()
}

async function dropFile(page: Page, name: string, contents: string) {
  await byTestId(page, 'skill-import-upload')
    .locator('input[type="file"]')
    .setInputFiles({
      name,
      mimeType: 'text/markdown',
      buffer: Buffer.from(contents, 'utf8'),
    })
}

test.describe('Skills — Import dialog', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToSkillsPage(page, testInfra.baseURL)
  })

  test('validating a valid SKILL.md shows the success result', async ({ page }) => {
    await mockValidate(page, { valid: true, errors: [], warnings: [] })
    await openImportDialog(page)
    await dropFile(page, 'SKILL.md', '---\nname: demo\n---\nHello')
    await byTestId(page, 'skill-import-validate-button').click()

    // tone="success" → the Alert renders role="status" (i18n-safe).
    const alert = byTestId(page, 'skill-import-validation-alert')
    await expect(alert).toBeVisible({ timeout: 5000 })
    await expect(alert).toHaveAttribute('role', 'status')
  })

  test('validating an invalid SKILL.md surfaces the error messages', async ({ page }) => {
    await mockValidate(page, {
      valid: false,
      errors: [{ code: 'E_FRONTMATTER', location: 'frontmatter', message: 'missing name' }],
      warnings: [],
    })
    await openImportDialog(page)
    await dropFile(page, 'SKILL.md', 'no frontmatter here')
    await byTestId(page, 'skill-import-validate-button').click()

    // tone="error" → role="alert"; the per-error message is dynamic data
    // (supplied by this test's mock), so asserting it on the alert is allowed.
    const alert = byTestId(page, 'skill-import-validation-alert')
    await expect(alert).toBeVisible({ timeout: 5000 })
    await expect(alert).toHaveAttribute('role', 'alert')
    await expect(alert).toContainText('frontmatter: missing name')
  })

  test('validating a non-md bundle shows the advisory instead of calling validate', async ({ page }) => {
    let validateCalled = false
    await page.route(/\/api\/skills\/validate$/, async route => {
      validateCalled = true
      return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' })
    })
    await openImportDialog(page)
    await dropFile(page, 'bundle.tar.gz', 'binary-bundle-bytes')
    await byTestId(page, 'skill-import-validate-button').click()

    // Non-md → the advisory toast is shown and /validate is never called.
    // (Toast copy is i18n chrome; assert the toast surfaced structurally and
    // that the inline validation alert never rendered.)
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({
      timeout: 5000,
    })
    await expect(byTestId(page, 'skill-import-validation-alert')).toHaveCount(0)
    expect(validateCalled).toBe(false)
  })
})
