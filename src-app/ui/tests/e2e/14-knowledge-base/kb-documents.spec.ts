import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

// TEST-38 (ITEM-31,13): the KB documents panel — empty state, upload a document,
// the per-doc status badge reaches `Indexed` (real FTS index path, no embedder),
// and removing it returns to empty. (The failed→retry + oversize-reject variants
// are covered by the unit/integration index-state tests.)
test.describe('Knowledge Base — documents panel', () => {
  test('upload → Indexed → remove', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const kb = await page.request.post(`${apiURL}/api/knowledge-bases`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: 'Docs KB' },
    })
    const kbId: string = (await kb.json()).id

    await page.goto(`${baseURL}/knowledge/${kbId}`)
    await expect(byTestId(page, 'kb-detail-title')).toBeVisible()
    await expect(byTestId(page, 'kb-documents-empty')).toBeVisible()

    // FB-4 / TEST-54: the count tag + Add-documents button live in the card
    // header (title + top-right extra), not a redundant body row under a
    // duplicate "Documents" heading.
    const docsCard = byTestId(page, 'kb-detail-documents')
    await expect(docsCard.getByTestId('kb-documents-count')).toHaveText(/0 documents/)
    await expect(docsCard.getByTestId('kb-documents-upload-button')).toBeVisible()

    // The Upload uses `directory` (webkitdirectory), so Playwright must be given
    // a directory PATH; write a temp dir holding one .txt and select it.
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'kb-docs-'))
    fs.writeFileSync(path.join(dir, 'protocol.txt'), 'indexed knowledge base document body')
    await page
      .locator('[data-testid="kb-documents-upload"] input[type="file"]')
      .setInputFiles(dir)

    // The doc row appears and its status badge reaches Indexed (live via
    // sync:file_index_state; FTS indexing needs no embedder).
    const statusBadge = page.locator('[data-testid^="kb-document-status-"]').first()
    await expect(statusBadge).toBeVisible({ timeout: 15_000 })
    await expect(statusBadge).toHaveText('Indexed', { timeout: 20_000 })

    // FB-3 / TEST-53: the document row reuses the shared FileCard component
    // (thumbnail + size/type subtitle), exactly like the project knowledge-files
    // panel — not a hand-rolled list row.
    await expect(page.locator('[data-testid="file-card"]').first()).toBeVisible()

    // Remove the document → back to empty.
    await page.locator('[data-testid^="kb-document-remove-"]').first().click()
    await expect(byTestId(page, 'kb-documents-empty')).toBeVisible({ timeout: 10_000 })
  })

  // FB-8 / TEST-56: the detail page lets a user VERIFY + trace the KB — a
  // retrieval-mode line, a "test retrieval" search box that returns hits, and a
  // "Used in" card. (Backend seeds one indexed doc via the real upload path.)
  test('detail page: retrieval mode line + test-retrieval search + used-in', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const kb = await page.request.post(`${apiURL}/api/knowledge-bases`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: 'Verify KB' },
    })
    const kbId: string = (await kb.json()).id
    const up = await page.request.post(`${apiURL}/api/files/upload`, {
      headers: { Authorization: `Bearer ${token}` },
      multipart: {
        file: {
          name: 'photosynthesis.txt',
          mimeType: 'text/plain',
          buffer: Buffer.from('photosynthesis converts light energy into chemical energy'),
        },
      },
    })
    await page.request.post(`${apiURL}/api/knowledge-bases/${kbId}/documents`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { file_ids: [(await up.json()).id] },
    })

    await page.goto(`${baseURL}/knowledge/${kbId}`)
    await expect(byTestId(page, 'kb-detail-title')).toBeVisible()

    // Retrieval-mode line in the Overview card.
    await expect(byTestId(page, 'kb-detail-overview')).toContainText('Retrieval')

    // "Test retrieval" search box returns the seeded passage (FTS, no embedder).
    await byTestId(page, 'kb-search-input').fill('photosynthesis')
    await byTestId(page, 'kb-search-button').click()
    await expect(byTestId(page, 'kb-search-hits')).toBeVisible({ timeout: 20_000 })
    await expect(page.locator('[data-testid^="kb-search-open-"]').first()).toBeVisible()

    // "Used in" card renders (empty — not attached anywhere yet).
    await expect(byTestId(page, 'kb-detail-used-in')).toBeVisible()
    await expect(byTestId(page, 'kb-detail-used-in-empty')).toBeVisible()
  })

  // FB-5/FB-6 / TEST-55: the documents list uses NUMBERED server-side pagination
  // (discrete pages via ListPagination, default page size 10 — like the
  // users/memories settings pages), NOT infinite scroll. Seed 12 docs → page 1
  // shows 10 + "1-10 of 12 documents"; page 2 shows the remaining 2.
  test('paginates the documents list with numbered pages (default 10)', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const kb = await page.request.post(`${apiURL}/api/knowledge-bases`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: 'Paging Docs KB' },
    })
    const kbId: string = (await kb.json()).id

    // Seed 12 documents (> the default page size of 10) so a 2nd page exists.
    const N = 12
    const fileIds: string[] = []
    for (let start = 0; start < N; start += 6) {
      const batch = []
      for (let i = start; i < Math.min(start + 6, N); i++) {
        batch.push(
          page.request.post(`${apiURL}/api/files/upload`, {
            headers: { Authorization: `Bearer ${token}` },
            multipart: {
              file: {
                name: `doc-${String(i).padStart(3, '0')}.txt`,
                mimeType: 'text/plain',
                buffer: Buffer.from(`knowledge document number ${i}`),
              },
            },
          }),
        )
      }
      for (const r of await Promise.all(batch)) {
        expect(r.ok()).toBeTruthy()
        fileIds.push((await r.json()).id)
      }
    }
    const attach = await page.request.post(
      `${apiURL}/api/knowledge-bases/${kbId}/documents`,
      { headers: { Authorization: `Bearer ${token}` }, data: { file_ids: fileIds } },
    )
    expect(attach.ok()).toBeTruthy()

    await page.goto(`${baseURL}/knowledge/${kbId}`)
    await expect(byTestId(page, 'kb-detail-title')).toBeVisible()

    // Page 1: exactly 10 of 12 rendered + the numbered pagination summary.
    await expect(byTestId(page, 'kb-documents-pagination')).toBeVisible({ timeout: 30_000 })
    await expect(page.locator('[data-testid="file-card"]')).toHaveCount(10)
    await expect(page.getByText(/1-10 of 12 documents/)).toBeVisible()

    // Go to page 2 → the remaining 2 documents (NOT appended — replaced).
    await page.getByRole('button', { name: 'Next page' }).click()
    await expect(page.getByText(/11-12 of 12 documents/)).toBeVisible({ timeout: 15_000 })
    await expect(page.locator('[data-testid="file-card"]')).toHaveCount(2)
  })
})
