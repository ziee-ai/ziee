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
  // Output-token budget. A REASONING bridge model (e.g. qwen3.6) spends its
  // budget on hidden reasoning before emitting the final `content`; too small a
  // cap gets it cut off mid-reasoning and it returns EMPTY content. Tool-driven
  // specs are fine on the small default (they emit a tool_call, not prose), but a
  // spec that reads the model's TEXT reply (goal-seeking) must give it headroom.
  maxTokens = 1536,
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
      parameters: { context_length: 16384, temperature: 0, top_p: 0.9, max_tokens: maxTokens },
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

/**
 * Seed a real provider + a tool-capable bridge model + an (empty) conversation
 * bound to that model, all owned by the acting user. Used by the background-run
 * specs (ITEM-8/9/10), which need a conversation whose `model_id` the detached
 * sub-agent runs on. Returns the ids the spec needs.
 */
export async function seedBridgeConversation(
  page: Page,
  apiURL: string,
  token: string,
  displayName: string,
): Promise<{ modelId: string; conversationId: string }> {
  // Imported lazily to keep this helper self-contained at the call sites.
  const { createProviderViaAPI, assignProviderToAdministratorsGroup } =
    await import('../../../common/provider-helpers')
  const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, token, providerId)
  const modelId = await createBridgeToolModel(page, apiURL, token, providerId, displayName)

  const convRes = await page.request.post(`${apiURL}/api/conversations`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { title: `${displayName} conversation`, model_id: modelId },
  })
  expect(convRes.ok()).toBeTruthy()
  const conversationId = (await convRes.json()).id as string
  return { modelId, conversationId }
}

/**
 * Launch a REAL detached background sub-agent by calling the built-in
 * `background_mcp` server's JSON-RPC endpoint directly (`POST /api/background/mcp`
 * with the `x-conversation-id` header the built-in reads). This drives the SAME
 * production `spawn_background` → `runner::spawn_background_run` → real bridge
 * sub-agent turn → terminal transition (`SyncEntity::WorkflowRun`) → completion
 * `notification` (`SyncEntity::Notification`) path the chat model would trigger,
 * but deterministically (no dependence on the model choosing to call the tool and
 * no per-run approval click). Returns the opaque owner-scoped `run_id`.
 */
export async function spawnBackgroundSubagent(
  page: Page,
  apiURL: string,
  token: string,
  conversationId: string,
  task: string,
): Promise<string> {
  const res = await page.request.post(`${apiURL}/api/background/mcp`, {
    headers: {
      Authorization: `Bearer ${token}`,
      'x-conversation-id': conversationId,
      'Content-Type': 'application/json',
    },
    data: {
      jsonrpc: '2.0',
      id: 1,
      method: 'tools/call',
      params: { name: 'spawn_background', arguments: { kind: 'subagent', spec: { task } } },
    },
  })
  expect(res.ok(), `spawn_background failed: ${res.status()} ${await res.text()}`).toBeTruthy()
  const body = await res.json()
  const runId = body?.result?.structuredContent?.run_id as string | undefined
  expect(runId, `no run_id in spawn_background result: ${JSON.stringify(body)}`).toBeTruthy()
  return runId as string
}
