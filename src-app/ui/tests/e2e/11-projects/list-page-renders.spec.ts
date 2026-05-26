import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  assertEmptyState,
  goToProjectsPage,
} from './helpers/project-helpers'

test.describe('Projects - List page render', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)
  })

  test('passes accessibility checks', async ({ page }) => {
    await assertNoAccessibilityViolations(page)
  })

  test('shows empty state when no projects exist', async ({ page }) => {
    await assertEmptyState(page)
    await expect(
      page.getByRole('button', { name: /create project/i }),
    ).toBeVisible()
  })

  test('header create-button is permission-gated and visible to admin', async ({
    page,
  }) => {
    // Admin has projects::create — the "+"-icon header button must be
    // present.
    await expect(
      page.getByRole('button', { name: /create project/i }).first(),
    ).toBeVisible()
  })
})
