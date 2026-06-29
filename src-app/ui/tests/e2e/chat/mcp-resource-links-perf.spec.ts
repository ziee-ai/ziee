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

// 1x1 transparent PNG — keeps ImageBody's <img> in DOM.
const TINY_PNG = Buffer.from(
  '89504E470D0A1A0A0000000D49484452000000010000000108060000001F15C4890000000D49444154789C6200010000050001' +
    '0D0A2DB40000000049454E44AE426082',
  'hex',
)

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

  test('20 image resource_links render within a 15s wall-clock budget', async ({
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
    // Wall-clock budget includes the ~3s seed setup + the 10s toHaveCount
    // timeout above; deliberately generous to absorb E2E cold-start variance.
    // This guards against a hard hang, not a fine-grained render-time SLA.
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
    await expect(preview.locator('[data-testid^="file-delimited-table-row-"]').first()).toBeVisible({ timeout: 10000 })
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
    await expect(previews.nth(0).locator('[data-testid^="file-delimited-table-row-"]').first()).toBeVisible()
    await expect(previews.nth(1).locator('[data-testid^="file-delimited-table-row-"]').first()).toBeVisible()
    expect(mock.callCount(u1)).toBe(1)
    expect(mock.callCount(u2)).toBe(1)
  })

  test('off-screen previews defer their body until scrolled into view', async ({
    page,
    testInfra,
  }) => {
    // A tall message with many files. Only previews near the viewport mount
    // their body (and thus fetch) — previews far away stay header-only until
    // scrolled to. This is the fix for laggy reloads that loaded every file at
    // once. (Position-agnostic: we don't assume where the page settles, only
    // that NOT ALL bodies mount, and that scrolling a gated one in mounts it.)
    const COUNT = 80
    const links = Array.from({ length: COUNT }, (_, i) => ({
      uri: `/api/files/gate-${i}/download`,
      name: `f-${i}.csv`,
      mime_type: 'text/csv',
    }))
    await seedAssistantWithToolResult(page, testInfra.baseURL, { resourceLinks: links })
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(COUNT, { timeout: 10000 })

    // Headers all render; bodies are viewport-gated. Once ANY body has mounted
    // (so the observers have run), the mounted set equals only the previews
    // within the viewport+800px band — never the off-screen tail. 80 collapsed
    // headers far exceed any headless viewport + the 800px margin, so the band
    // cannot contain all of them regardless of where the page settled. No
    // fixed sleep needed: the off-screen previews never enter the band, so the
    // count cannot later grow to COUNT without an explicit scroll.
    const bodies = page.locator('[data-testid="inline-file-preview-body"]')
    await expect(bodies.first()).toBeVisible({ timeout: 10000 })
    const mounted = await bodies.count()
    expect(mounted).toBeGreaterThan(0)
    expect(mounted).toBeLessThan(COUNT)

    // Scrolling a previously-gated preview into view mounts its body.
    await previews.last().scrollIntoViewIfNeeded()
    await expect(
      previews.last().locator('[data-testid="inline-file-preview-body"]'),
    ).toHaveCount(1, { timeout: 10000 })
  })

  test('inline images use native lazy loading + async decoding', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/lazy-img/download'
    await mockResourceLinkUrl(page, uri, TINY_PNG, { contentType: 'image/png' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'p.png', mime_type: 'image/png' }],
    })
    const img = page.locator('[data-testid="inline-file-preview"] img').first()
    await expect(img).toBeVisible({ timeout: 10000 })
    await expect(img).toHaveAttribute('loading', 'lazy')
    await expect(img).toHaveAttribute('decoding', 'async')
  })
})
