import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Document-RAG user JOURNEY stitched across surfaces in one flow:
 *   1. Admin RAG settings page is reachable and shows the config (Document
 *      search toggle + capability-filtered embedding picker).
 *   2. A project is created and a knowledge file is uploaded + attached via the
 *      combined endpoint.
 *   3. The attached file renders on the project detail page (the knowledge that
 *      a RAG-grounded chat would later retrieve over).
 *
 * The grounded-answer retrieval leg needs a real embedder and is covered by the
 * Tier-3 backend tests (see the note in `18-file-rag/admin-surface.spec.ts`);
 * this journey covers the UI spine that precedes it, which no single spec did.
 */

test.describe('Document RAG — configure + upload + attach journey', () => {
  test('admin config → project file upload → file shows in knowledge panel', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // ── Leg 1: RAG admin config surface is reachable. ──────────────────
    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(byTestId(page, 'filerag-enable-card')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'filerag-embedding-card')).toBeVisible()

    // ── Leg 2: create a project + upload/attach a knowledge file. ──────
    const projRes = await page.request.post(`${apiURL}/api/projects`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: `RAG Journey ${Date.now()}` },
    })
    expect(projRes.ok()).toBeTruthy()
    const projectId = (await projRes.json()).id as string

    const status = await page.evaluate(
      async ([apiBase, pid, t]) => {
        const fd = new FormData()
        fd.append(
          'file',
          new Blob(['Ziee supports retrieval-augmented generation over documents.'], {
            type: 'text/plain',
          }),
          'rag-notes.txt',
        )
        const r = await fetch(`${apiBase}/api/projects/${pid}/files/upload`, {
          method: 'POST',
          headers: { Authorization: `Bearer ${t}` },
          body: fd,
        })
        return r.status
      },
      [apiURL, projectId, token] as const,
    )
    expect(status).toBe(201)

    // ── Leg 3: the file renders on the project detail knowledge panel. ──
    await page.goto(`${baseURL}/projects/${projectId}`)
    await expect(
      page.locator('[data-testid="file-card"][data-filename="rag-notes.txt"]'),
    ).toBeVisible({ timeout: 15000 })
  })
})
