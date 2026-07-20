import type { Page } from '@playwright/test'
import { expect } from '@playwright/test'

/**
 * Shared setup for the agent-orchestration real-LLM E2E specs.
 *
 * These specs drive the REAL agent-core chat loop (ZIEE_CHAT_AGENT_CORE=1) against
 * a real OpenAI-compatible bridge (e.g. a local Qwen served on :4000). They need a
 * TOOL-CAPABLE model row (`capabilities.tools=true`) so the agent-core built-in
 * tools (task_*, delegate, background/spawn, …) are actually offered — otherwise
 * the model has nothing to call and the live surfaces never render.
 *
 * The bridge wiring reuses the same env seam as `createProviderViaAPI`
 * (OPENAI_BASE_URL + OPENAI_API_KEY) plus the model-name seam ZIEE_TEST_LLM_MODEL,
 * so nothing is hardcoded and the whole family skips cleanly when the bridge env
 * is unset.
 */

export const BRIDGE_BASE_URL = process.env.OPENAI_BASE_URL || process.env.ZIEE_TEST_LLM_BASE_URL
export const BRIDGE_MODEL = process.env.ZIEE_TEST_LLM_MODEL || process.env.OPENAI_MODEL
export const HAS_BRIDGE = Boolean(BRIDGE_BASE_URL && BRIDGE_MODEL)
export const BRIDGE_SKIP =
  'no real LLM endpoint (set OPENAI_BASE_URL + OPENAI_API_KEY + ZIEE_TEST_LLM_MODEL)'

/**
 * Create a tool-capable model row pointed at the bridge model. `createModelViaAPI`
 * forces `function_calling:false`, so we POST the row directly with
 * `capabilities.tools=true` (mirrors `run-js-real-llm.spec.ts`). The provider row
 * already carries the bridge base_url + key (via `createProviderViaAPI`).
 */
export async function createBridgeToolModel(
  page: Page,
  apiURL: string,
  token: string,
  providerId: string,
  displayName: string,
): Promise<string> {
  const res = await page.request.post(`${apiURL}/api/llm-models`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      provider_id: providerId,
      name: BRIDGE_MODEL,
      display_name: displayName,
      enabled: true,
      engine_type: 'none',
      file_format: 'gguf',
      capabilities: { tools: true, chat: true, streaming: true },
      parameters: { context_length: 16384, temperature: 0, top_p: 0.9, max_tokens: 1536 },
    },
  })
  expect(res.ok()).toBeTruthy()
  const body = await res.json()
  return body.id as string
}

/**
 * Enable the deployment-wide agent admin toggles the orchestration surfaces need.
 * `PUT /api/agent/settings` is the singleton COALESCE-patch (validated). The
 * `delegate_enabled` flag is what makes the agent-core chat path offer the core
 * `delegate` (fan-out) tool (ITEM-2 / DEC-2); other knobs (goal_seek_max_turns,
 * fan-out caps) are patched only when a spec needs them.
 */
export async function updateAgentAdminSettings(
  page: Page,
  apiURL: string,
  token: string,
  patch: Record<string, unknown>,
): Promise<void> {
  const res = await page.request.put(`${apiURL}/api/agent/settings`, {
    headers: { Authorization: `Bearer ${token}` },
    data: patch,
  })
  expect(res.ok()).toBeTruthy()
}
