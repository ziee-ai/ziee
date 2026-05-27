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

  test('drawer surfaces default-assistant + default-model pickers', async ({
    page,
  }) => {
    await openCreateProjectDrawer(page)
    // Both pickers are present even when no options exist — they render
    // as Select dropdowns with "No default" placeholder text.
    await expect(page.getByLabel(/default assistant/i)).toBeVisible()
    await expect(page.getByLabel(/default model/i)).toBeVisible()
  })
})
