import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  ALL_STEP_KINDS,
  addStep,
  openNewBuilder,
  saveBuilder,
  waitBuilderValid,
} from './helpers/builder-helpers'

/**
 * TEST-12 — the add-step kind picker + the schema-driven per-kind config forms.
 *
 *   the picker offers all 6 kinds → add a Tool step + an Llm step → each renders
 *   TYPED fields (a server Select / a tool Input / key-value args; a prompt +
 *   an Output segmented) — NOT a single raw-JSON textarea → an invalid tool step
 *   surfaces inline field validation → a valid config Saves. The llm form has NO
 *   tools/capability picker (the backend rejects `tools:` on an llm step).
 *
 * No API mocking: a real personal MCP server is seeded via the REST API so the
 * tool step's server Select has a real option to choose.
 */

test.describe('Workflows — builder step kinds + typed forms', () => {
  test('all 6 kinds; tool+llm typed forms, inline validation, valid saves', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const wfName = `e2e-builder-kinds-${Date.now()}`
    const srvName = `e2e_tool_srv_${Date.now()}`
    const srvDisplay = 'E2E Tool Server'

    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed a real, enabled, user-owned MCP server so the tool step's server
    // picker has an option (health check is disabled in E2E, so it stays on).
    const srvResp = await request.post(`${apiURL}/api/mcp/servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: srvName,
        display_name: srvDisplay,
        enabled: true,
        transport_type: 'http',
        url: 'https://tool-srv.example.invalid/mcp',
        timeout_seconds: 30,
      },
    })
    expect(srvResp.status(), `seed mcp server: ${await srvResp.text()}`).toBe(201)

    await openNewBuilder(page, baseURL)

    // The kind picker offers all 6 kinds.
    await byTestId(page, 'wf-builder-add-step-btn').click()
    for (const kind of ALL_STEP_KINDS) {
      await expect(
        byTestId(page, `wf-builder-add-step-menu-item-${kind}`),
      ).toBeVisible()
    }
    // Close the menu (Escape) before adding via the helper.
    await page.keyboard.press('Escape')

    // ── Tool step: typed form + inline validation ────────────────────────────
    const toolId = await addStep(page, 'tool', 1) // tool_1
    const cfg = byTestId(page, 'wf-builder-step-config')
    // Typed fields, NOT a raw JSON box: a server Select (combobox), a tool
    // Input, and a key/value argument editor.
    await expect(byTestId(page, 'wf-builder-tool-server')).toBeVisible()
    await expect(byTestId(page, 'wf-builder-tool-name')).toBeVisible()
    await expect(byTestId(page, 'wf-builder-tool-arg-add')).toBeVisible()
    // A fresh tool step (empty server + tool) surfaces inline required-field
    // validation right in the form.
    await expect(cfg).toContainText('A server is required')
    await expect(cfg).toContainText('A tool name is required')

    // Make it valid: pick the seeded server + type a tool name.
    await byTestId(page, 'wf-builder-tool-server').click()
    await byTestId(page, `wf-builder-tool-server-opt-${srvName}`).click()
    await byTestId(page, 'wf-builder-tool-name').fill('search')
    // The inline errors clear once the fields are filled.
    await expect(cfg).not.toContainText('A tool name is required')

    // ── Llm step: typed form, NO tools picker ────────────────────────────────
    const llmId = await addStep(page, 'llm', 1) // llm_1
    // Typed fields: a prompt textarea + an Output segmented control.
    await expect(byTestId(page, 'wf-builder-llm-prompt')).toBeVisible()
    await expect(byTestId(page, 'wf-builder-llm-output')).toBeVisible()
    // The llm form has NO capability/tools picker (that is agent-only; the
    // backend rejects `tools:` on an llm step).
    await expect(byTestId(page, 'wf-builder-agent-servers')).toHaveCount(0)
    await byTestId(page, 'wf-builder-llm-prompt').fill('Summarize the result.')

    // Sanity: the two steps exist and carry their kind tags.
    await expect(byTestId(page, `wf-builder-step-kind-${toolId}`)).toContainText(
      'Call a tool',
    )
    await expect(byTestId(page, `wf-builder-step-kind-${llmId}`)).toContainText(
      'LLM prompt',
    )

    // A valid config Saves.
    await byTestId(page, 'wf-builder-name').fill(wfName)
    await waitBuilderValid(page)
    await saveBuilder(page)
    await expect(page).toHaveURL(/\/settings\/workflows\/[0-9a-f-]+\/edit$/, {
      timeout: 15000,
    })
  })
})
