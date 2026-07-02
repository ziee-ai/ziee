import { test, expect } from '../../fixtures/test-context'
import { getAdminToken, loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  fillProjectForm,
  getProjectCard,
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
    await getProjectCard(page, 'Knowledge Target').click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    // Knowledge section is now an inline block (section[data-test-section="knowledge"])
    // — verify the empty-state marker is present in the compact preview.
    await expect(
      page.locator('[data-test-files-empty="true"]'),
    ).toBeVisible()

    // "Manage" button is always present, even when there are no files
    // (so users can navigate to attach their first one).
    await expect(
      byTestId(page, 'project-knowledge-manage-button'),
    ).toBeVisible()
  })

  test('uploads file via combined endpoint and shows in panel', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Drive directly to the project's detail page via its known card.
    await getProjectCard(page, 'Knowledge Target').click()
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

    // Refresh; the detail page's knowledge section now renders an
    // inline grid of square FileCards via ProjectFilesInlinePreview
    // (the empty-state is replaced by the cards once any file
    // attaches). Verify both surfaces — the inline preview AND the
    // Manage drawer — see the new file.
    await page.reload()

    // Inline preview shows the file card directly on the detail page.
    await expect(
      page.locator('[data-testid="file-card"][data-filename="notes.txt"]'),
    ).toBeVisible({ timeout: 10000 })

    // Manage drawer (full row layout) also shows the file.
    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer = page.getByRole('dialog')
    await drawer.waitFor({ state: 'visible' })
    await expect(
      drawer.locator('[data-testid="file-card"][data-filename="notes.txt"]'),
    ).toBeVisible()
  })

  test('inline preview uses responsive CSS grid and stretches FileCards', async ({
    page,
    testInfra,
  }) => {
    // Upload a few files via the combined endpoint and assert the
    // inline preview block on the detail page renders them in a CSS
    // grid (not flex-wrap) with `auto-fill minmax(<min>, 1fr)`. Cards
    // are FileCard's square variant in `stretch` mode — the wrapper
    // no longer hard-codes 96px width; the grid track owns the size.
    const { baseURL } = testInfra
    await getProjectCard(page, 'Knowledge Target').click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    const projectId = page.url().split('/').pop()!
    const token = await getAdminToken(baseURL)

    // Three files so the grid has enough children to lay out a row.
    for (const name of ['a.txt', 'b.txt', 'c.txt']) {
      const status = await page.evaluate(
        async ([apiBase, pid, t, fname]) => {
          const fd = new FormData()
          fd.append('file', new Blob(['x'], { type: 'text/plain' }), fname)
          const r = await fetch(`${apiBase}/api/projects/${pid}/files/upload`, {
            method: 'POST',
            headers: { Authorization: `Bearer ${t}` },
            body: fd,
          })
          return r.status
        },
        [baseURL, projectId, token, name],
      )
      expect(status, `upload ${name}`).toBe(201)
    }

    await page.reload()

    // Locate the inline preview's grid container — the only direct
    // child of the Knowledge section that has `display: grid`.
    const knowledge = page.locator('[data-test-section="knowledge"]')
    await expect(knowledge).toBeVisible()
    // Wait for at least one card to render so the grid container has
    // been mounted with its inline-style.
    await expect(
      knowledge.locator('[data-testid="file-card"]').first(),
    ).toBeVisible({ timeout: 10000 })

    // All three cards present.
    await expect(knowledge.locator('[data-testid="file-card"]')).toHaveCount(3)

    // Find the grid wrapper (descendant of the section with grid
    // display); assert the `grid-template-columns` reflects the
    // auto-fill + minmax pattern from ProjectFilesInlinePreview.
    const gridLayoutOk = await knowledge.evaluate(section => {
      const grid = section.querySelector(
        ':scope div[style*="grid-template-columns"]',
      ) as HTMLElement | null
      if (!grid) return { found: false, reason: 'no grid container' }
      const cs = window.getComputedStyle(grid)
      return {
        found: true,
        display: cs.display,
        // The inline style is parsed to a computed string of px tracks
        // by the browser; assert the inline-style raw attribute too
        // so we can pin the auto-fill pattern.
        inlineCols: grid.getAttribute('style'),
      }
    })
    expect(gridLayoutOk.found).toBe(true)
    expect(gridLayoutOk.display).toBe('grid')
    expect(gridLayoutOk.inlineCols ?? '').toMatch(/auto-fill/)
    expect(gridLayoutOk.inlineCols ?? '').toMatch(/minmax/)

    // FileCard square + stretch mode: the wrapper does NOT have the
    // legacy `width: 96px` inline style. Reading the first card's
    // wrapper to confirm width is NOT pinned at 96px.
    const cardWidthFixed96 = await knowledge
      .locator('[data-testid="file-card"]')
      .first()
      .evaluate(el => el.style.width === '96px')
    expect(cardWidthFixed96).toBe(false)
  })

  test('row-variant delete requires Popconfirm — Cancel preserves the file', async ({
    page,
    testInfra,
  }) => {
    // Regression for the "click trash, file deletes immediately"
    // bug. Manage drawer's row-variant FileCard now wraps its delete
    // button in a Popconfirm; Cancel must not delete.
    const { baseURL } = testInfra
    await getProjectCard(page, 'Knowledge Target').click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    const projectId = page.url().split('/').pop()!
    const token = await getAdminToken(baseURL)

    const status = await page.evaluate(
      async ([apiBase, pid, t]) => {
        const fd = new FormData()
        fd.append('file', new Blob(['x'], { type: 'text/plain' }), 'doomed.txt')
        const r = await fetch(`${apiBase}/api/projects/${pid}/files/upload`, {
          method: 'POST',
          headers: { Authorization: `Bearer ${t}` },
          body: fd,
        })
        return r.status
      },
      [baseURL, projectId, token],
    )
    expect(status).toBe(201)

    await page.reload()
    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer = page.getByRole('dialog')
    await drawer.waitFor({ state: 'visible' })
    const row = () =>
      drawer.locator('[data-testid="file-card"][data-filename="doomed.txt"]')
    await expect(row()).toBeVisible()

    // Click the row's delete button — should open the Confirm
    // (AlertDialog), NOT delete immediately.
    await row().locator('[data-testid^="file-project-delete-btn-"]').click({ force: true })
    const confirm = page.getByRole('alertdialog')
    await expect(confirm).toBeVisible({ timeout: 5000 })

    // Cancel (Escape dismisses the Confirm) — the file must remain.
    await page.keyboard.press('Escape')
    await expect(confirm).toBeHidden()
    await expect(row()).toBeVisible()

    // Reopen + confirm via the Confirm's primary (Delete) button.
    await row().locator('[data-testid^="file-project-delete-btn-"]').click({ force: true })
    await expect(confirm).toBeVisible()
    await confirm.locator('[data-testid$="-confirm"]').click()

    // File disappears after confirmed delete.
    await expect(row()).toHaveCount(0, { timeout: 5000 })
  })

  test('uploads a file through the real Manage-drawer Upload control', async ({
    page,
  }) => {
    // Drive the actual antd <Upload> control (not page.evaluate + fetch): the
    // beforeUpload handler → uploadAndAttachFiles → combined upload endpoint.
    await getProjectCard(page, 'Knowledge Target').click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer = page.getByRole('dialog')
    await drawer.waitFor({ state: 'visible' })

    // The Upload control renders a hidden <input type="file"> — set a real
    // file on it (the genuine UI upload path, exercising beforeUpload).
    await drawer.locator('input[type="file"]').setInputFiles({
      name: 'ui-upload.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('uploaded through the real Upload control'),
    })

    // The uploaded file appears in the drawer's knowledge-file list.
    await expect(
      drawer.locator('[data-testid="file-card"][data-filename="ui-upload.txt"]'),
    ).toBeVisible({ timeout: 15000 })
  })
})
