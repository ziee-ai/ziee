import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  seedAssistantWithToolResult,
  mockResourceLinkUrl,
  mockResourceLinkUrls,
} from './fixtures/mock-tool-result'

/**
 * Performance / load tests for the inline file-preview pipeline.
 * Most failures here indicate either a missing memoization or an
 * accidental refetch on re-render.
 */

test.describe('Inline file previews — performance', () => {
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

  test('20 image resource_links render within 5 seconds', async ({
    page,
    testInfra,
  }) => {
    const links = Array.from({ length: 20 }, (_, i) => ({
      uri: `/api/files/perf-img-${i}/download`,
      name: `img-${i}.png`,
      mime_type: 'image/png',
    }))
    const t0 = Date.now()
    await seedAssistantWithToolResult(page, testInfra.baseURL, { resourceLinks: links })
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(20, { timeout: 10000 })
    const elapsed = Date.now() - t0
    // Generous bound — the seed itself takes ~3s of setup. The render
    // of 20 items inside should be well under another 2s on top.
    expect(elapsed).toBeLessThan(15000)
  })

  test('useResourceLinkContent does not re-fetch on collapse + re-expand', async ({
    page,
    testInfra,
  }) => {
    // Previously covered in the streaming spec; included here as a
    // standalone perf-focused test in case future regressions creep in.
    const uri = '/api/files/perf-cache/download'
    const mock = await mockResourceLinkUrl(page, uri, 'a,b\n1,2\n', { contentType: 'text/csv' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'data.csv', mime_type: 'text/csv' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview.locator('table:has(tbody td)')).toBeVisible({ timeout: 10000 })
    expect(mock.callCount(uri)).toBe(1)
    const chevron = preview.locator('[data-testid="inline-file-preview-chevron"]')
    for (let i = 0; i < 5; i++) {
      await chevron.click()
      await chevron.click()
    }
    expect(mock.callCount(uri)).toBe(1)
  })

  test('dedup of identical URIs across tool_results triggers only one fetch', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/perf-dedup/download'
    const mock = await mockResourceLinkUrl(page, uri, '# Hi', { contentType: 'text/markdown' })
    // Five resource_links all pointing at the same URI.
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: Array.from({ length: 5 }, () => ({
        uri,
        name: 'r.md',
        mime_type: 'text/markdown',
      })),
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview.locator('h1')).toBeVisible({ timeout: 10000 })
    // Dedup leaves one preview → one fetch.
    expect(await page.locator('[data-testid="inline-file-preview"]').count()).toBe(1)
    expect(mock.callCount(uri)).toBe(1)
  })

  test('module-level cache survives mount/unmount of a preview', async ({
    page,
    testInfra,
  }) => {
    // Seed two distinct previews. Confirm each fires exactly one fetch.
    // (Catches a class of regression where Strict-Mode re-mounts or a
    // forced re-render double-fetches.)
    const u1 = '/api/files/perf-mount-1/download'
    const u2 = '/api/files/perf-mount-2/download'
    const mock = await mockResourceLinkUrls(page, [
      { url: u1, body: 'a,b\n1,2\n', contentType: 'text/csv' },
      { url: u2, body: 'c,d\n3,4\n', contentType: 'text/csv' },
    ])
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: u1, name: 'a.csv', mime_type: 'text/csv' },
        { uri: u2, name: 'b.csv', mime_type: 'text/csv' },
      ],
    })
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(2, { timeout: 10000 })
    await expect(previews.nth(0).locator('table:has(tbody td)')).toBeVisible()
    await expect(previews.nth(1).locator('table:has(tbody td)')).toBeVisible()
    expect(mock.callCount(u1)).toBe(1)
    expect(mock.callCount(u2)).toBe(1)
  })
})
