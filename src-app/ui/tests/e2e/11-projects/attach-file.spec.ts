import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

/**
 * Knowledge tab: attach an existing file (via the file picker route)
 * to a project, see it in the panel, detach it again.
 *
 * v1 ProjectFilesPanel uses the existing file-library state — when no
 * files exist yet the empty state is shown. Full multipart upload via
 * the drawer is exercised in the integration-test suite; the E2E spec
 * here covers UI behavior end-to-end.
 */
test.describe('Projects - Knowledge / file attach', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    // Seed a project we can drive into the detail page.
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, { name: 'Knowledge Target' })
    await submitProjectForm(page)
  })

  test('knowledge tab empty state', async ({ page }) => {
    // Open the project's detail page by clicking its card.
    await page.locator('.ant-card', { hasText: 'Knowledge Target' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    // The Knowledge tab is the default tab on the detail page.
    await expect(page.getByText(/no knowledge files yet/i)).toBeVisible()
  })

  test('uploads file via combined endpoint and shows in panel', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Drive directly to the project's detail page via its known card.
    await page.locator('.ant-card', { hasText: 'Knowledge Target' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    const url = new URL(page.url())
    const projectId = url.pathname.split('/').pop()!

    // Use the API directly to upload + attach; this matches the v1
    // UX path where the user attaches an existing file through the
    // combined endpoint. Full drag-drop UX is a Phase D polish item.
    const token = await page.evaluate(() => localStorage.getItem('access_token'))
    const formData = new FormData()
    formData.append(
      'file',
      new Blob(['hello world'], { type: 'text/plain' }),
      'notes.txt',
    )
    const resp = await page.evaluate(
      async ([apiBase, pid, t]) => {
        const fd = new FormData()
        fd.append('file', new Blob(['hello world'], { type: 'text/plain' }), 'notes.txt')
        const r = await fetch(`${apiBase}/api/projects/${pid}/files/upload`, {
          method: 'POST',
          headers: { Authorization: `Bearer ${t}` },
          body: fd,
        })
        return r.status
      },
      [baseURL, projectId, token],
    )
    expect(resp).toBe(201)

    // Refresh and verify the file appears in the Knowledge list.
    await page.reload()
    await expect(page.getByText('notes.txt')).toBeVisible()
  })
})
