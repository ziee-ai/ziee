import { test, expect } from '../../fixtures/test-context'
import { getAdminToken, loginAsAdmin } from '../../common/auth-helpers'
import {
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

/**
 * Round-4 Option A redesign: Knowledge is now an inline compact
 * preview on the project detail page (chip strip + "Manage" button
 * that opens the full file panel in a Drawer). No more "Knowledge
 * tab" — the empty state lives directly in the preview block.
 *
 * Full multipart upload via the drawer is exercised in the
 * integration-test suite; the E2E spec here covers UI behavior
 * end-to-end (empty-state visibility + post-attach chip rendering).
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

  test('inline knowledge preview shows empty state on a new project', async ({
    page,
  }) => {
    await page.locator('.ant-card', { hasText: 'Knowledge Target' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    // Knowledge section is now an inline block (section[data-test-section="knowledge"])
    // — verify the empty-state marker is present in the compact preview.
    await expect(
      page.locator('[data-test-files-empty="true"]'),
    ).toBeVisible()

    // "Manage" button is always present, even when there are no files
    // (so users can navigate to attach their first one).
    await expect(
      page.getByRole('button', { name: /manage knowledge files/i }),
    ).toBeVisible()
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
    //
    // Fetch the token via the dedicated helper — `localStorage`'s
    // `access_token` key never existed; the auth store persists under
    // `auth-storage` and tests should not couple to that internal
    // shape.
    const token = await getAdminToken(baseURL)
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

    // Refresh and verify the file appears in the inline knowledge
    // preview as a chip. The data-test-file-chip attribute is
    // stable across UI label tweaks.
    await page.reload()
    await expect(
      page.locator('[data-test-file-chip="notes.txt"]'),
    ).toBeVisible()

    // The "Manage" button still works — opening the drawer should
    // show the same file in the detailed list.
    await page
      .getByRole('button', { name: /manage knowledge files/i })
      .click()
    await page.locator('.ant-drawer.ant-drawer-open').waitFor({ state: 'visible' })
    await expect(
      page
        .locator('.ant-drawer.ant-drawer-open')
        .getByText('notes.txt')
        .first(),
    ).toBeVisible()
  })
})
