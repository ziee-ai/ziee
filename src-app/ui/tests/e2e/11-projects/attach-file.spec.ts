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
    await page.locator('.ant-card', { hasText: 'Knowledge Target' }).click()
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
    await page.locator('.ant-card', { hasText: 'Knowledge Target' }).click()
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
    await page
      .getByRole('button', { name: /manage knowledge files/i })
      .click()
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await drawer.waitFor({ state: 'visible' })
    await expect(drawer.getByText('doomed.txt').first()).toBeVisible()

    // Click the row's delete button — should trigger Popconfirm, NOT delete.
    await drawer
      .getByRole('button', { name: /^delete doomed\.txt$/i })
      .click()

    // Popconfirm renders as a portal — match the title text.
    const confirmPopover = page.locator('.ant-popover-content', {
      hasText: 'Delete this file?',
    })
    await expect(confirmPopover).toBeVisible({ timeout: 5000 })

    // Click Cancel — the file must remain.
    await confirmPopover.getByRole('button', { name: /^cancel$/i }).click()
    // Drawer still open, file still listed.
    await expect(drawer.getByText('doomed.txt').first()).toBeVisible()

    // Now confirm via the Popconfirm's Delete button. Move the mouse
    // off the trash button first so the lingering antd Tooltip
    // ("Delete") dismisses — otherwise the tooltip layer sits over
    // the popover and intercepts pointer events when the popover's
    // Delete button is in roughly the same screen region.
    await page.mouse.move(10, 10)
    await drawer
      .getByRole('button', { name: /^delete doomed\.txt$/i })
      .click()
    await page.mouse.move(10, 10)
    await expect(confirmPopover).toBeVisible()
    await confirmPopover
      .getByRole('button', { name: /^delete$/i })
      .click({ force: true })

    // File disappears after confirmed delete.
    await expect(drawer.getByText('doomed.txt')).toHaveCount(0, {
      timeout: 5000,
    })
  })
})
