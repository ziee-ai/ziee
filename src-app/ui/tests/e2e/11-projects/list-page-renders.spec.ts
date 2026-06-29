import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
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
    // The empty-state CTA (its own testid) renders distinct from the
    // icon-only header "+" CTA.
    await expect(
      byTestId(page, 'project-list-empty-create-button'),
    ).toBeVisible()
  })

  test('header create-button is permission-gated and visible to admin', async ({
    page,
  }) => {
    // Admin has projects::create — the icon-only "+" header button
    // (its own testid) must be present.
    await expect(byTestId(page, 'project-list-create-button')).toBeVisible()
  })
})
