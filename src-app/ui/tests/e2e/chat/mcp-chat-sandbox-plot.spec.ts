import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown, sendChatMessage } from './helpers/chat-helpers'
import { MockResourceLinkServer } from '../mcp/helpers/resource-link-mock-server'

/**
 * LLM-gated end-to-end for the inline file-preview feature.
 *
 * Uses a real Anthropic LLM + a Node mock MCP server (no real sandbox)
 * exposing a single `get_file_link` tool. The LLM is given a plain-
 * English request and should:
 *   1. Decide to call `get_file_link` with the right name/mime_type.
 *   2. Receive a `resource_link` content block.
 *   3. The frontend `MessageFilesView` (the tool_result content
 *      renderer) picks it up and renders an `InlineFilePreview` matched to
 *      the file's MIME via the viewer registry.
 *
 * Mocks the underlying file URL fetch via page.route so the test
 * doesn't need a real backend artifact.
 *
 * Skips cleanly without ANTHROPIC_API_KEY.
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('Chat — LLM-driven inline file preview (real LLM + mock MCP server)', () => {
  test.skip(!HAS_ANTHROPIC_KEY, 'ANTHROPIC_API_KEY not set — skipping LLM-gated tests')
  test.slow()

  let mock: MockResourceLinkServer

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    mock = await MockResourceLinkServer.start({ baseUrl: baseURL })

    // Register mock as a system MCP server.
    const created = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: `mock_resource_link_${Date.now()}`,
        display_name: 'Mock File Links',
        description: 'Node mock that returns resource_link content blocks',
        enabled: true,
        transport_type: 'http',
        url: mock.url(),
        timeout_seconds: 120,
        usage_mode: 'auto',
      },
    })
    const serverBody = await created.json()
    const serverId: string = serverBody.id

    // Assign to default group.
    const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    const groupsBody = await groupsRes.json()
    const groups: Array<{ id: string; is_default?: boolean; name: string }> =
      Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
    const defaultGroup = groups.find(g => g.is_default) ?? groups.find(g => g.name === 'Users')
    if (defaultGroup) {
      await page.request.post(`${apiURL}/api/mcp/system-servers/${serverId}/groups`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { group_ids: [defaultGroup.id] },
      })
    }

    // Auto-approve so the LLM-driven tool call doesn't block on user
    // approval — same pattern as the sampling spec.
    await page.request.put(`${apiURL}/api/mcp/defaults`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        selected_servers: [{ server_id: serverId, tools: [] }],
        disabled_servers: [],
        approval_mode: 'auto_approve',
        auto_approved_tools: [],
      },
    })
  })

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('LLM-driven PNG round-trip renders inline <img> in chat', async ({
    page,
    testInfra,
  }) => {
    // Intercept the resource_link URL the mock will return. Use a `**`
    // glob because `page.route` with a plain string matches the full
    // URL, not just the path (see mock-tool-result.ts comment).
    const mockedUri = '/api/files/mock/plot.png'
    await page.route(`**${mockedUri}`, async route => {
      // Tiny 1x1 PNG (transparent).
      const png = Buffer.from(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
        'base64',
      )
      await route.fulfill({ status: 200, contentType: 'image/png', body: png })
    })

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Use the get_file_link tool to make a PNG named "plot.png" with mime_type "image/png" available, then show it to me.',
      true,
    )

    // Wait up to 60s for the inline preview to appear.
    const img = page
      .locator('[data-testid="inline-file-preview"] img')
      .first()
    await expect(img).toBeVisible({ timeout: 60000 })
    expect(mock.toolCallCount()).toBeGreaterThan(0)
  })

  test('LLM-driven CSV round-trip renders inline <table> in chat', async ({
    page,
    testInfra,
  }) => {
    const mockedUri = '/api/files/mock/data.csv'
    await page.route(`**${mockedUri}`, async route => {
      await route.fulfill({
        status: 200,
        contentType: 'text/csv',
        body: 'a,b\n1,2\n3,4\n5,6\n',
      })
    })

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Use the get_file_link tool with name "data.csv" and mime_type "text/csv" to make a small dataset available to me. Show me the file.',
      true,
    )

    const table = page
      .locator('[data-testid="inline-file-preview"] table')
      .first()
    await expect(table).toBeVisible({ timeout: 60000 })
    expect(mock.toolCallCount()).toBeGreaterThan(0)
  })

  test('LLM-driven multi-file round-trip renders both inline', async ({
    page,
    testInfra,
  }) => {
    await page.route('**/api/files/mock/img.png', async route => {
      const png = Buffer.from(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
        'base64',
      )
      await route.fulfill({ status: 200, contentType: 'image/png', body: png })
    })
    await page.route('**/api/files/mock/data.csv', async route => {
      await route.fulfill({
        status: 200,
        contentType: 'text/csv',
        body: 'x,y\n10,20\n30,40\n',
      })
    })

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Call get_file_link TWICE: once with name "img.png"/mime_type "image/png", once with name "data.csv"/mime_type "text/csv". Show me both files.',
      true,
    )

    // Both previews should render.
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(2, { timeout: 60000 })
    // One has an <img>, the other a <table>.
    await expect(page.locator('[data-testid="inline-file-preview"] img').first()).toBeVisible()
    await expect(page.locator('[data-testid="inline-file-preview"] table').first()).toBeVisible()
  })

  test('LLM-driven markdown round-trip renders inline rendered markdown', async ({
    page,
    testInfra,
  }) => {
    await page.route('**/api/files/mock/report.md', async route => {
      await route.fulfill({
        status: 200,
        contentType: 'text/markdown',
        body: '# Quarterly Report\n\n**Revenue** increased 17%.\n',
      })
    })

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Use get_file_link with name "report.md" and mime_type "text/markdown" to surface a markdown report. Show me.',
      true,
    )

    const heading = page
      .locator('[data-testid="inline-file-preview"] h1')
      .first()
    await expect(heading).toBeVisible({ timeout: 60000 })
    await expect(heading).toHaveText(/Quarterly Report/)
  })

  test('LLM-driven plain text log round-trip renders <pre>', async ({
    page,
    testInfra,
  }) => {
    await page.route('**/api/files/mock/log.txt', async route => {
      await route.fulfill({
        status: 200,
        contentType: 'text/plain',
        body: '2026-05-25 INFO startup\n2026-05-25 INFO ready\n',
      })
    })

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Use get_file_link with name "log.txt" and mime_type "text/plain" to surface a log file. Show me.',
      true,
    )

    // RawCodeView (used by the text viewer) renders a div-based
    // line-numbered layout, not a <pre>. Anchor on its data-testid.
    const rawView = page
      .locator('[data-testid="inline-file-preview"] [data-testid="raw-code-view"]')
      .first()
    await expect(rawView).toBeVisible({ timeout: 60000 })
    await expect(rawView).toContainText('startup')
  })
})

// ──────────────────────────────────────────────────────────────────────────

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}
