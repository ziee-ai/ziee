import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  assertProjectExists,
  assertSuccessMessage,
  clickCardAction,
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
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

    // Click the inline Edit icon button on the card (round-3 ProjectCard
    // rewrite replaced the Dropdown menu with inline icon buttons).
    await clickCardAction(page, 'Edit Target', 'Edit')

    // The drawer should be in Edit mode with prefilled values (the
    // prefilled name/description prove edit-mode hydration).
    await expect(byTestId(page, 'project-form-name-input')).toHaveValue(
      'Edit Target',
    )
    await expect(
      byTestId(page, 'project-form-description-textarea'),
    ).toHaveValue('Original')

    // Update + save.
    await byTestId(page, 'project-form-description-textarea').fill(
      'Updated description',
    )
    await byTestId(page, 'project-form-instructions-textarea').fill(
      'Speak in haiku.',
    )
    await submitProjectForm(page)
    await assertSuccessMessage(page, /project updated/i)
  })
})
