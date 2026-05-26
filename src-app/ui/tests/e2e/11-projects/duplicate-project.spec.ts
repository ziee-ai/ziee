import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  assertProjectExists,
  clickCardMenuItem,
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  openProjectCardMenu,
  submitProjectForm,
} from './helpers/project-helpers'

test.describe('Projects - Duplicate', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)
  })

  test('duplicates a project from the card menu', async ({ page }) => {
    // Seed.
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, {
      name: 'Dup Source',
      description: 'Will be cloned',
    })
    await submitProjectForm(page)
    await assertProjectExists(page, 'Dup Source')

    // Open the card menu → Duplicate.
    await openProjectCardMenu(page, 'Dup Source')
    await clickCardMenuItem(page, 'Duplicate')

    // Server appends " (copy)" to the name.
    await assertProjectExists(page, 'Dup Source (copy)')
    // Original still visible.
    await assertProjectExists(page, 'Dup Source')
  })

  test('duplicates from the detail-page header button', async ({ page }) => {
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, { name: 'Header Dup' })
    await submitProjectForm(page)

    // Open detail page.
    await page.locator('.ant-card', { hasText: 'Header Dup' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    // Click "Duplicate" in the header bar.
    await page
      .getByRole('button', { name: /^duplicate$/i })
      .first()
      .click()

    // The page navigates to the new project's detail page.
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/, { timeout: 10000 })
    await expect(page.getByText(/Header Dup \(copy\)/i)).toBeVisible()
  })
})
