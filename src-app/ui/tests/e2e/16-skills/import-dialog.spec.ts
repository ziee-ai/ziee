import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'

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
  await page.getByRole('button', { name: /import/i }).click()
  await expect(page.getByRole('dialog').getByText('Import Skill')).toBeVisible()
}

async function dropFile(page: Page, name: string, contents: string) {
  await page.locator('input[type="file"]').setInputFiles({
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
    await page.getByRole('button', { name: 'Validate' }).click()

    await expect(page.getByText('Valid skill')).toBeVisible({ timeout: 5000 })
  })

  test('validating an invalid SKILL.md surfaces the error messages', async ({ page }) => {
    await mockValidate(page, {
      valid: false,
      errors: [{ code: 'E_FRONTMATTER', location: 'frontmatter', message: 'missing name' }],
      warnings: [],
    })
    await openImportDialog(page)
    await dropFile(page, 'SKILL.md', 'no frontmatter here')
    await page.getByRole('button', { name: 'Validate' }).click()

    await expect(page.getByText('Validation failed')).toBeVisible({ timeout: 5000 })
    await expect(page.getByText(/frontmatter: missing name/)).toBeVisible()
  })

  test('validating a non-md bundle shows the advisory instead of calling validate', async ({ page }) => {
    let validateCalled = false
    await page.route(/\/api\/skills\/validate$/, async route => {
      validateCalled = true
      return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' })
    })
    await openImportDialog(page)
    await dropFile(page, 'bundle.tar.gz', 'binary-bundle-bytes')
    await page.getByRole('button', { name: 'Validate' }).click()

    await expect(page.getByText(/Validation reads SKILL\.md text/i)).toBeVisible({ timeout: 5000 })
    expect(validateCalled).toBe(false)
  })
})
