import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  assertProjectExists,
  assertSuccessMessage,
  clickCardMenuItem,
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  openProjectCardMenu,
  submitProjectForm,
} from './helpers/project-helpers'

test.describe('Projects - Edit project', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)
  })

  test('edits an existing project and persists the change', async ({
    page,
  }) => {
    // Seed a project via the drawer.
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, {
      name: 'Edit Target',
      description: 'Original',
    })
    await submitProjectForm(page)
    await assertProjectExists(page, 'Edit Target')

    // Open the card menu → Edit.
    await openProjectCardMenu(page, 'Edit Target')
    await clickCardMenuItem(page, 'Edit')

    // The drawer should be in Edit mode with prefilled values.
    await expect(
      page.locator('.ant-drawer.ant-drawer-open').getByText(/edit project/i),
    ).toBeVisible()
    await expect(page.getByLabel('Name')).toHaveValue('Edit Target')
    await expect(page.getByLabel('Description')).toHaveValue('Original')

    // Update + save.
    await page.getByLabel('Description').fill('Updated description')
    await page.getByLabel('Instructions').fill('Speak in haiku.')
    await submitProjectForm(page)
    await assertSuccessMessage(page, /project updated/i)
  })
})
