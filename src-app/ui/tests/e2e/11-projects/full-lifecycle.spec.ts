import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * E2E — the FULL project lifecycle in ONE spec (no single existing spec combines
 * more than 2-3 steps): create + configure a project → send a message in it →
 * receive a real LLM response that proves the project context reached the model
 * → MANAGE the project (rename via the detail header). Real-LLM tier — soft-skip
 * without ANTHROPIC_API_KEY.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''

test.describe('Projects — full lifecycle (real LLM)', () => {
  test.skip(ANTHROPIC_KEY.length === 0, 'ANTHROPIC_API_KEY not set — real-LLM lifecycle skipped')

  test('create → configure → send → receive → manage (rename)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // 1) Configure: real Anthropic provider + Haiku model.
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

    // 2) Create + configure a project with a beacon instruction.
    const projRes = await page.request.post(`${apiURL}/api/projects`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: 'Lifecycle Project',
        instructions:
          "Begin every response with the exact literal string 'ZZZ_LIFECYCLE_99' (no preface).",
      },
    })
    expect(projRes.ok()).toBeTruthy()
    const projectId = (await projRes.json()).id as string

    // 3) Send a message in the project's inline composer.
    await page.goto(`${baseURL}/projects/${projectId}`)
    await page.waitForLoadState('load')
    const textarea = page.locator(
      '[data-test-section="chat-input"] textarea[placeholder*="Type your message"]',
    )
    await expect(textarea).toBeVisible({ timeout: 10000 })
    await textarea.fill('Say hello.')
    const send = page.getByRole('button', { name: 'Send message' })
    await expect(send).toBeEnabled({ timeout: 10000 })
    await send.click()

    // 4) Receive: the streamed response carries the project's beacon.
    await page.waitForURL(new RegExp(`/projects/${projectId}/chat/[0-9a-f-]+`), {
      timeout: 30000,
    })
    await expect(page.locator('body')).toContainText('ZZZ_LIFECYCLE_99', {
      timeout: 45000,
    })

    // 5) Manage: rename the project via the detail-page Edit drawer.
    await page.goto(`${baseURL}/projects/${projectId}`)
    await page.getByRole('button', { name: /^edit$/i }).first().click()
    const drawer = page.locator(
      '.ant-drawer.ant-drawer-open:has-text("Edit Project")',
    )
    await expect(drawer).toBeVisible({ timeout: 10000 })
    await drawer.getByLabel('Name').fill('Lifecycle Project (renamed)')
    await drawer.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(page.getByText('Project updated')).toBeVisible({ timeout: 10000 })
    await expect(
      page.getByRole('heading', {
        level: 4,
        name: /Lifecycle Project \(renamed\)/,
      }),
    ).toBeVisible({ timeout: 15000 })
  })
})
