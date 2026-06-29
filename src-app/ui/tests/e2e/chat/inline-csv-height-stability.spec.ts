import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  mockBackendFile,
  seedAssistantWithToolResult,
} from './fixtures/mock-tool-result'

/**
 * Inline CSV preview — height stability.
 *
 * Regression guard for the shrink-to-zero bug: the CSV/TSV viewer body
 * (`DelimitedTable`) measures its own container with a `ResizeObserver` and
 * feeds the result into the antd virtual table's `scroll.y`. Inline, the
 * preview wrapper only imposed a `max-height`, so the table's `height: 100%`
 * resolved to `auto` and the measurement looped toward zero. The fix gives
 * `inlineFill` viewers a DEFINITE-height inline box, so the body holds a stable,
 * non-tiny height. (Fails on the pre-fix code; passes after.)
 */
test.describe('Inline CSV preview — height stays stable (no shrink-to-zero)', () => {
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

  test('CSV artifact body holds a stable, non-tiny height', async ({
    page,
    testInfra,
  }) => {
    const fileId = 'file-csv-stability'
    // Enough rows that the (buggy) feedback loop has ample height to collapse.
    const lines = ['name,age,city']
    for (let i = 0; i < 60; i++) lines.push(`Person${i},${20 + i},City${i}`)
    const csv = lines.join('\n')

    // Backend-owned artifact: entity + /text endpoint return the CSV content.
    await mockBackendFile(page, {
      fileId,
      filename: 'data.csv',
      mimeType: 'text/csv',
      textContent: csv,
    })

    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        {
          uri: `/api/files/${fileId}/download`,
          name: 'data.csv',
          mime_type: 'text/csv',
          file_id: fileId,
        },
      ],
    })

    const body = page.locator('[data-testid="inline-file-preview-body"]').first()
    await expect(body).toBeVisible({ timeout: 15000 })
    // The data grid renders inside the inline body.
    await expect(byTestId(body, 'file-delimited-table')).toBeVisible({ timeout: 15000 })

    // Let any ResizeObserver-driven layout settle — the bug needs a few frames
    // to converge toward zero.
    await page.waitForTimeout(1500)
    const h1 = (await body.boundingBox())?.height ?? 0
    // Fixed inline height is 420px; anything healthy is well above 300.
    expect(h1).toBeGreaterThan(300)

    // It must NOT keep shrinking afterwards.
    await page.waitForTimeout(1000)
    const h2 = (await body.boundingBox())?.height ?? 0
    expect(h2).toBeGreaterThan(300)
    expect(Math.abs(h2 - h1)).toBeLessThan(20)
  })
})
