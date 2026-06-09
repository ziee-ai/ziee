import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  seedAssistantWithToolResult,
  mockBackendFile,
} from './fixtures/mock-tool-result'

/**
 * Backend-owned artifacts (resource_link carrying `file_id`).
 *
 * These exercise the consolidated single-view path: the inline preview
 * renders content through the authenticated `/api/files/{id}/...` endpoints
 * (the same path the right-side panel uses) rather than dereferencing the
 * tool-emitted absolute loopback `uri`, and the header gains an
 * "Open in side panel" button.
 *
 * Covers the two original bugs together:
 *  - Bug 1 (text extraction): `/api/files/{id}/text` returns the `.R` source,
 *    so the code viewer is not blank.
 *  - Bug 2 (inline URL): content loads even though the `uri` is an unreachable
 *    `http://127.0.0.1:…` URL — proving `file_id` drives the fetch.
 */
test.describe('Inline file previews — backend-owned artifacts (file_id)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('.R artifact renders source inline via the authenticated file path', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const fileId = 'art-r-1'
    const rSource = 'x <- 1:10\nprint(mean(x))\n'
    await mockBackendFile(page, {
      fileId,
      filename: 'analysis.R',
      mimeType: 'text/plain',
      textContent: rSource,
    })

    const { assistantMessageId } = await seedAssistantWithToolResult(page, baseURL, {
      resourceLinks: [
        {
          // Unreachable absolute loopback URI the tool emits — must NOT be used
          // now that file_id drives the fetch.
          uri: 'http://127.0.0.1:9999/api/code-sandbox/file/download?filename=analysis.R',
          name: 'analysis.R',
          mime_type: 'text/plain',
          file_id: fileId,
        },
      ],
    })

    const msg = page.locator(
      `[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`,
    )
    const preview = msg.locator('[data-testid="inline-file-preview"]')
    await expect(preview).toBeVisible()

    // Body renders the R source (Bug 1: extraction returned content; Bug 2:
    // fetched via /api/files/{id}/text, not the loopback uri).
    const code = preview.locator('[data-testid="raw-code-view"]')
    await expect(code).toBeVisible()
    await expect(code).toContainText('print(mean(x))')
    await expect(preview).not.toContainText('Failed to load file content.')

    // Single view: no duplicate file-card.
    await expect(msg.locator('[data-testid="file-card"]')).toHaveCount(0)

    // Both header buttons are present for a backend-owned file.
    await expect(
      preview.locator('[data-testid="inline-file-preview-open-panel"]'),
    ).toBeVisible()
    await expect(
      preview.locator('[data-testid="inline-file-preview-open"]'),
    ).toBeVisible()
  })

  test('CSV artifact renders as a table inline via file_id', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const fileId = 'art-csv-1'
    await mockBackendFile(page, {
      fileId,
      filename: 'data.csv',
      mimeType: 'text/csv',
      textContent: 'city,pop\nHanoi,8000000\n',
    })

    const { assistantMessageId } = await seedAssistantWithToolResult(page, baseURL, {
      resourceLinks: [
        {
          uri: 'http://127.0.0.1:9999/api/code-sandbox/file/download?filename=data.csv',
          name: 'data.csv',
          mime_type: 'text/csv',
          file_id: fileId,
        },
      ],
    })

    const preview = page
      .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
      .locator('[data-testid="inline-file-preview"]')
    await expect(preview).toBeVisible()
    // AntD Table v6 renders dual `<table>` elements for fixed-header
    // scroll (one in the header, one in the body) — strict-mode-safe
    // selector targets only the BODY table where data rows live.
    const table = preview.locator('.ant-table-row').first()
    await expect(table).toBeVisible()
    await expect(table).toContainText('Hanoi')
    await expect(preview).not.toContainText('Failed to load file content.')
  })

  test('non-inline artifact (PDF) shows a header-only row with both buttons', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const fileId = 'art-pdf-1'
    // No textContent: the PDF viewer is not inline-capable, so the preview is
    // header-only — the consolidated replacement for the old file-card.
    await mockBackendFile(page, {
      fileId,
      filename: 'report.pdf',
      mimeType: 'application/pdf',
    })

    const { assistantMessageId } = await seedAssistantWithToolResult(page, baseURL, {
      resourceLinks: [
        {
          uri: 'http://127.0.0.1:9999/api/code-sandbox/file/download?filename=report.pdf',
          name: 'report.pdf',
          mime_type: 'application/pdf',
          file_id: fileId,
        },
      ],
    })

    const preview = page
      .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
      .locator('[data-testid="inline-file-preview"]')
    await expect(preview).toBeVisible()
    await expect(preview).toContainText('report.pdf')
    // No inline body for a non-inline-capable viewer.
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
    // Both header buttons present.
    await expect(
      preview.locator('[data-testid="inline-file-preview-open-panel"]'),
    ).toBeVisible()
    await expect(
      preview.locator('[data-testid="inline-file-preview-open"]'),
    ).toBeVisible()
  })

  test('"Open in side panel" opens the file in the right panel', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const fileId = 'art-r-2'
    await mockBackendFile(page, {
      fileId,
      filename: 'script.R',
      mimeType: 'text/plain',
      textContent: 'cat("hi from R\\n")\n',
    })

    const { assistantMessageId } = await seedAssistantWithToolResult(page, baseURL, {
      resourceLinks: [
        {
          uri: 'http://127.0.0.1:9999/api/code-sandbox/file/download?filename=script.R',
          name: 'script.R',
          mime_type: 'text/plain',
          file_id: fileId,
        },
      ],
    })

    const msg = page.locator(
      `[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`,
    )
    await msg.locator('[data-testid="inline-file-preview-open-panel"]').click()

    await expect(page.locator('[data-testid="chat-right-panel"]')).toHaveAttribute(
      'data-panel-open',
      'true',
    )
  })
})
