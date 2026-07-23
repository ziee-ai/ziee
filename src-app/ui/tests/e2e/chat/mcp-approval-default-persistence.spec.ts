import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { createConversationWithModel, goToNewChatPage } from './helpers/chat-helpers'

/**
 * "MCP auto-approve doesn't survive past turn 1."
 *
 * The approval mode a brand-new conversation gets is the SERVER's decision. The
 * client used to hardcode `manual_approve`: it showed that in the config modal and
 * then PERSISTED it on the first send, so a deployment configured to auto-approve
 * ran turn 1's tool with no prompt (no stored row → server default) and prompted
 * from turn 2 on (client-written row saying manual).
 *
 * Every assertion here compares against `GET /api/mcp/defaults`.`default_approval_mode`
 * rather than a literal. That is deliberate: this spec is shared with
 * `deploy-schedule`, where the server default is `auto_approve` instead of
 * `manual_approve`. A literal would either pass vacuously on one branch or fail on
 * the other; comparing to the server's own advertised value proves the real
 * property — the client no longer decides — on both.
 */

/** The model this spec creates and picks. Only used to get a conversation minted. */
const MODEL_DISPLAY_NAME = 'Approval Default Model'

test.describe('MCP approval default — persistence across turns', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Provider + model so the chat page can actually send. The upstream call
    // fails (fake key), which is fine — the conversation is minted and the
    // client's turn-1 auto-persist fires on the send, not on the reply.
    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    // Display name pinned explicitly so the model picker below matches a value
    // this spec controls, rather than the helper's default (which an env
    // override like ZIEE_TEST_LLM_MODEL can change out from under it).
    await createModelViaAPI(apiURL, token, providerId, undefined, MODEL_DISPLAY_NAME, 'openai')
  })

  // ── TEST-20: the literal reported repro ────────────────────────────────────

  test('a first message does not pin an approval mode the server did not choose', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(page)

    const serverDefault = await getServerDefaultApprovalMode(page, apiURL, token)

    // Precondition: no user defaults row, so the conversation's mode can only
    // come from the server default. Without this the test could pass by
    // inheriting a stored user preference instead.
    const before = await getMcpDefaults(page, apiURL, token)
    expect(before.defaults ?? null).toBeNull()

    // The reported repro: a real new chat, a real first send.
    const conversationId = await createConversationWithModel(
      page,
      baseURL,
      MODEL_DISPLAY_NAME,
      'first message that does not use any tool',
    )

    // The client's onMessageSent auto-persist is fire-and-forget; wait for the
    // row rather than racing it.
    const settings = await waitForConversationMcpSettings(page, apiURL, token, conversationId)

    expect(
      settings.approval_mode,
      `the turn-1 auto-persist must not override the server default ` +
        `(${serverDefault}); a later turn reads THIS row, which is why the ` +
        `conversation used to start prompting from message 2 on`,
    ).toBe(serverDefault)
  })

  // ── TEST-21: the second, wider clobber path ────────────────────────────────

  test('removing a server chip on a new chat records the server list without pinning a mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(page)

    const serverDefault = await getServerDefaultApprovalMode(page, apiURL, token)
    const serverId = await createSystemServer(page, apiURL, token, `Default Chip ${Date.now()}`)

    // Still NO user-defaults row: with none, McpInitializer selects every enabled
    // server, so the chip appears without us seeding a mode first.
    const before = await getMcpDefaults(page, apiURL, token)
    expect(before.defaults ?? null).toBeNull()

    await goToNewChatPage(page, baseURL)
    const chip = page.locator(`[data-testid="mcp-chip-${serverId}"]`)
    await expect(chip).toBeVisible({ timeout: 15000 })

    // Remove it through the real UI — this is what writes the user-defaults row.
    await chip.getByTestId(`mcp-chip-${serverId}-close`).click()
    await expect(chip).not.toBeVisible({ timeout: 10000 })

    const after = await waitForUserDefaults(page, apiURL, token)

    // The removal must be recorded …
    expect(
      after.disabled_servers?.some((d: { server_id: string }) => d.server_id === serverId),
      `the chip removal must persist to the user's default server list`,
    ).toBe(true)

    // … WITHOUT the write also choosing an approval mode. This row is the
    // fallback for every FUTURE conversation, so a mode pinned here is far
    // wider-reaching than the per-conversation clobber above.
    expect(
      after.approval_mode,
      `a server-list side effect must not set the user's default approval mode ` +
        `(expected the server default ${serverDefault})`,
    ).toBe(serverDefault)
  })

  // ── TEST-22: what the user is TOLD matches what the server will do ─────────

  test('the config modal on a new chat shows the server default approval mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(page)

    const serverDefault = await getServerDefaultApprovalMode(page, apiURL, token)
    // An enabled server so the "+" menu exposes the MCP item at all.
    await createSystemServer(page, apiURL, token, `Default Modal ${Date.now()}`)

    await goToNewChatPage(page, baseURL)
    await byTestId(page, 'chat-input-add-btn').first().click()
    await byTestId(page, 'chat-mcp-menu-item').first().click()
    await expect(byTestId(page, 'mcp-config-modal')).toBeVisible({ timeout: 10000 })

    // Assert the rendered DOM, not the store: only a real render proves the user
    // is not being shown "Manual" on an auto-approving deployment.
    const select = byTestId(page, 'mcp-config-approval-select')
    await expect(select).toBeVisible()
    await expect(select).toContainText(APPROVAL_MODE_LABEL[serverDefault], {
      timeout: 10000,
    })
  })
})

// ──────────────────────────────────────────────────────────────────────────
// Local helpers
// ──────────────────────────────────────────────────────────────────────────

/** The user-visible label each mode renders as in the config modal's Select. */
const APPROVAL_MODE_LABEL: Record<string, string> = {
  disabled: 'Disabled',
  auto_approve: 'Auto Approve',
  manual_approve: 'Manual Approve',
}

type Page = import('@playwright/test').Page

async function getAdminToken(page: Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

async function getMcpDefaults(
  page: Page,
  apiURL: string,
  token: string,
): Promise<{ defaults: Record<string, unknown> | null; default_approval_mode: string }> {
  const res = await page.request.get(`${apiURL}/api/mcp/defaults`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  expect(res.ok()).toBe(true)
  return await res.json()
}

/**
 * The server's own default. Read, never hardcoded — see the file header.
 */
async function getServerDefaultApprovalMode(
  page: Page,
  apiURL: string,
  token: string,
): Promise<string> {
  const body = await getMcpDefaults(page, apiURL, token)
  const mode = body.default_approval_mode
  expect(
    Object.keys(APPROVAL_MODE_LABEL),
    'the server must advertise a valid default approval mode',
  ).toContain(mode)
  return mode
}

/**
 * Poll for the conversation's MCP settings row. `onMessageSent` persists it
 * without awaiting, so an immediate GET can legitimately return `settings: null`.
 * Fails loudly on timeout rather than silently asserting against a null row.
 */
async function waitForConversationMcpSettings(
  page: Page,
  apiURL: string,
  token: string,
  conversationId: string,
): Promise<Record<string, string>> {
  for (let attempt = 0; attempt < 30; attempt++) {
    const res = await page.request.get(
      `${apiURL}/api/conversations/${conversationId}/mcp-settings`,
      { headers: { Authorization: `Bearer ${token}` } },
    )
    if (res.ok()) {
      const body = await res.json()
      if (body.settings) return body.settings
    }
    await page.waitForTimeout(500)
  }
  throw new Error(
    `conversation ${conversationId} never got an mcp_settings row — the turn-1 ` +
      `auto-persist did not run, so this test cannot say anything about what it wrote`,
  )
}

/** Poll for the user-defaults row the chip removal creates. */
async function waitForUserDefaults(
  page: Page,
  apiURL: string,
  token: string,
): Promise<{ approval_mode: string; disabled_servers?: Array<{ server_id: string }> }> {
  for (let attempt = 0; attempt < 30; attempt++) {
    const body = await getMcpDefaults(page, apiURL, token)
    if (body.defaults) {
      return body.defaults as unknown as {
        approval_mode: string
        disabled_servers?: Array<{ server_id: string }>
      }
    }
    await page.waitForTimeout(500)
  }
  throw new Error('the chip removal never persisted a user-defaults row')
}

async function createSystemServer(
  page: Page,
  apiURL: string,
  token: string,
  displayName: string,
): Promise<string> {
  const res = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      name: `approval_default_${Date.now()}`,
      display_name: displayName,
      description: 'approval-default e2e fixture',
      enabled: true,
      transport_type: 'http',
      url: 'https://approval-default.example.invalid/mcp',
      timeout_seconds: 30,
      supports_sampling: false,
      usage_mode: 'auto',
    },
  })
  expect(res.ok()).toBe(true)
  const body = await res.json()
  await assignServerToAdminGroup(page, apiURL, token, body.id)
  return body.id
}

async function assignServerToAdminGroup(
  page: Page,
  apiURL: string,
  token: string,
  serverId: string,
): Promise<void> {
  const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!groupsRes.ok()) return
  const groupsBody = await groupsRes.json()
  const groups: Array<{ id: string; is_protected?: boolean; name: string }> =
    Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
  const adminGroup =
    groups.find(g => g.name === 'Administrators') ??
    groups.find(g => g.is_protected) ??
    groups[0]
  if (!adminGroup) return

  await page.request.post(`${apiURL}/api/mcp/system-servers/${serverId}/groups`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { group_ids: [adminGroup.id] },
  })
}
