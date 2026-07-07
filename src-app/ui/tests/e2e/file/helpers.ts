import type { Page } from '@playwright/test'
import { expect } from '../../fixtures/test-context'
import { getAdminToken } from '../../common/auth-helpers'
import {
  fillProjectForm,
  goToProjectsPage,
  getProjectCard,
  openCreateProjectDrawer,
  submitProjectForm,
} from '../projects/helpers/project-helpers'

/**
 * Create a project, upload a single file into its knowledge via the combined
 * endpoint, and land on the detail page with the file's inline FileCard visible.
 * Returns the file's id (for URL / new-tab assertions).
 *
 * Mirrors tests/e2e/projects/attach-file.spec.ts — the real upload path — so the
 * FilePreviewDrawer opens over a genuinely-persisted file.
 */
export async function seedProjectFile(
  page: Page,
  baseURL: string,
  opts: { projectName: string; filename: string; content: string; mime: string },
): Promise<string> {
  await goToProjectsPage(page, baseURL)
  await openCreateProjectDrawer(page)
  await fillProjectForm(page, { name: opts.projectName })
  await submitProjectForm(page)

  await getProjectCard(page, opts.projectName).click()
  await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
  const projectId = new URL(page.url()).pathname.split('/').pop()!

  const token = await getAdminToken(baseURL)
  const fileId = await page.evaluate(
    async ([apiBase, pid, t, name, content, mime]) => {
      const fd = new FormData()
      fd.append('file', new Blob([content], { type: mime }), name)
      const r = await fetch(`${apiBase}/api/projects/${pid}/files/upload`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${t}` },
        body: fd,
      })
      if (r.status !== 201) throw new Error(`upload failed: ${r.status}`)
      const body = await r.json()
      // The combined endpoint returns the created File (id on the file object).
      return body.id ?? body.file?.id ?? body.files?.[0]?.id
    },
    [baseURL, projectId, token, opts.filename, opts.content, opts.mime],
  )

  await page.reload()
  await expect(
    page.locator(`[data-testid="file-card"][data-filename="${opts.filename}"]`),
  ).toBeVisible({ timeout: 15000 })
  return fileId as string
}

/** Click the inline FileCard for `filename` (default click → FilePreviewDrawer)
 *  and return the opened drawer dialog locator. */
export async function openPreviewDrawer(page: Page, filename: string) {
  await page
    .locator(`[data-testid="file-card"][data-filename="${filename}"]`)
    .first()
    .click()
  const drawer = page.getByRole('dialog')
  await drawer.waitFor({ state: 'visible' })
  return drawer
}
