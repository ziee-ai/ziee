import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  assertProjectExists,
  clickCardAction,
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
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

    // Click the inline Duplicate icon button on the card (round-3
    // ProjectCard rewrite — Dropdown menu was replaced by inline icons).
    await clickCardAction(page, 'Dup Source', 'Duplicate')

    // Server appends " (copy)" to the name.
    await assertProjectExists(page, 'Dup Source (copy)')
    // Original still visible.
    await assertProjectExists(page, 'Dup Source')
  })

  test('duplicating twice disambiguates with a "(copy 2)" suffix', async ({
    page,
  }) => {
    // Seed the source.
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, { name: 'Collide Source' })
    await submitProjectForm(page)
    await assertProjectExists(page, 'Collide Source')

    // First duplicate → "Collide Source (copy)".
    await clickCardAction(page, 'Collide Source', 'Duplicate')
    await assertProjectExists(page, 'Collide Source (copy)')

    // Second duplicate of the SAME source → "(copy)" is taken, so the server
    // picks the next free "(copy N)" suffix → "Collide Source (copy 2)".
    await clickCardAction(page, 'Collide Source', 'Duplicate')
    await assertProjectExists(page, 'Collide Source (copy 2)')

    // All three coexist.
    await assertProjectExists(page, 'Collide Source')
    await assertProjectExists(page, 'Collide Source (copy)')
  })

  test('duplicates from the detail-page header button', async ({ page }) => {
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, { name: 'Header Dup' })
    await submitProjectForm(page)

    // Open detail page.
    await page.locator('.ant-card', { hasText: 'Header Dup' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    // Click "Duplicate" in the header bar. Note: antd icons (CopyOutlined)
    // contribute "copy" to the accessible name, so the button's full
    // accessible name is "copy Duplicate" — match by trailing
    // "Duplicate" rather than anchoring with /^…$/.
    await page
      .getByRole('button', { name: /duplicate/i })
      .first()
      .click()

    // The page navigates to the new project's detail page. Verify by
    // checking the H4 page title — the project name also appears in
    // the sidebar widget AND a brief success toast ("Duplicated as
    // …"), which would trip strict-mode if we matched plain text.
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/, { timeout: 10000 })
    await expect(
      page.getByRole('heading', { level: 4, name: /Header Dup \(copy\)/i }),
    ).toBeVisible()
  })
})
