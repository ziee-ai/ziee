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
    // Two "Create project" buttons can render at once: the icon-only
    // header CTA (aria-label "Create project") AND the larger
    // empty-state CTA (accessible name "folder-add Create Project"
    // due to the FolderAddOutlined icon). Verify the empty-state CTA
    // specifically by matching the primary-button variant — the
    // header CTA is `type="text"` so it does NOT have .ant-btn-primary.
    await expect(
      page.locator('.ant-btn-primary').filter({ hasText: /create project/i }),
    ).toBeVisible()
  })

  test('header create-button is permission-gated and visible to admin', async ({
    page,
  }) => {
    // Admin has projects::create — the icon-only "+" header button
    // (aria-label "Create project", exact) must be present. Match
    // exact form to avoid colliding with the empty-state CTA whose
    // accessible name is "folder-add Create Project".
    await expect(
      page.getByRole('button', { name: 'Create project', exact: true }),
    ).toBeVisible()
  })
})
