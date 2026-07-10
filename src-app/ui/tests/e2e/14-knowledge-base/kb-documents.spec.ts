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

    // Remove the document → back to empty.
    await page.locator('[data-testid^="kb-document-remove-"]').first().click()
    await expect(byTestId(page, 'kb-documents-empty')).toBeVisible({ timeout: 10_000 })
  })
})
