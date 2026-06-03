import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  assertProjectExists,
  assertSuccessMessage,
  cancelProjectForm,
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

test.describe('Projects - Create drawer', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)
  })

  test('opens drawer from empty-state CTA', async ({ page }) => {
    await openCreateProjectDrawer(page)
    await expect(
      page.locator('.ant-drawer.ant-drawer-open').getByText(/new project/i),
    ).toBeVisible()
  })

  test('creates a project with name + description + instructions', async ({
    page,
  }) => {
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, {
      name: 'E2E Test Project',
      description: 'Created via Playwright',
      instructions: 'Be concise.',
    })
    await submitProjectForm(page)
    await assertSuccessMessage(page, /project created/i)
    await assertProjectExists(page, 'E2E Test Project')
  })

  test('validates required name field', async ({ page }) => {
    await openCreateProjectDrawer(page)
    // Submit empty form. Ant form rules show inline error.
    await page
      .locator('.ant-drawer.ant-drawer-open')
      .getByRole('button', { name: /create/i })
      .click()
    await expect(page.getByText(/name is required/i)).toBeVisible()
    await expect(page.locator('.ant-drawer.ant-drawer-open')).toBeVisible()
  })

  test('cancel does not create the project', async ({ page }) => {
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, { name: 'Cancelled Project' })
    await cancelProjectForm(page)
    await assertProjectExists(page, 'Cancelled Project', false)
  })

  // NOTE: The default-assistant and default-model pickers used to live
  // in the create drawer; they were intentionally moved to the project
  // detail page's Advanced card (ProjectDefaultsForm) so they auto-save
  // inline rather than being tied to a create/edit form transaction. The
  // pickers' presence on the detail page is asserted by
  // detail-page-layout.spec.ts's "Advanced section summarises defaults"
  // test. The prior assertion against the drawer was deleted with that
  // refactor and is intentionally not replaced here.
})
