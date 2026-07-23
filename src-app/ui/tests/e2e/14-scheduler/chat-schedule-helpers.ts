import type { Page } from '@playwright/test'
import { expect } from '../../fixtures/test-context'

import { getAdminToken } from '../../common/auth-helpers'
import {
  assignProviderToAdministratorsGroup,
  createModelViaAPI,
  createProviderViaAPI,
} from '../../common/provider-helpers'
import { byTestId } from '../testid'

/**
 * Shared REAL-BACKEND seed for the in-chat "Schedule or loop this chat" specs
 * (TEST-81/85/90/92). Unlike the standalone `14-scheduler/*` specs — which mock
 * the scheduler REST endpoints — these drive the WHOLE flow against the live
 * backend, because the assertions are about the real bind (`bound_conversation_id`)
 * and the one-source-of-truth persistence, which mocking would hollow out.
 *
 * The scheduler `create_task` handler VALIDATES both `model_id` (must exist AND be
 * accessible to the user) and `bound_conversation_id` (must exist AND be owned),
 * so both a real, accessible model and a real, owned conversation must be seeded
 * — a fake UUID would 404/403.
 */

export interface ChatScheduleSeed {
  adminToken: string
  modelId: string
  conversationId: string
  providerName: string
}

/**
 * Seed a real accessible model + a real owned (empty) conversation for the acting
 * admin. Returns the ids the in-chat dialog + verification need.
 */
export async function seedModelAndConversation(
  page: Page,
  apiURL: string,
): Promise<ChatScheduleSeed> {
  const adminToken = await getAdminToken(apiURL)

  // A real provider + model the admin can access (validate_model_access).
  const providerName = `sched-prov-${Date.now().toString(36)}`
  const providerId = await createProviderViaAPI(apiURL, adminToken, providerName, 'openai')
  await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
  const modelId = await createModelViaAPI(
    apiURL,
    adminToken,
    providerId,
    undefined,
    undefined,
    'openai',
  )

  // A real, owned, empty conversation to bind to (POST needs no model / messages).
  const convRes = await page.request.post(`${apiURL}/api/conversations`, {
    headers: { Authorization: `Bearer ${adminToken}` },
    data: { title: `In-chat schedule ${Date.now().toString(36)}` },
  })
  if (!convRes.ok()) {
    throw new Error(`seed conversation failed: ${convRes.status()} ${await convRes.text()}`)
  }
  const conversationId = (await convRes.json()).id as string

  return { adminToken, modelId, conversationId, providerName }
}

/**
 * Navigate to the bound conversation and open the in-chat schedule/loop dialog via
 * its composer toolbar button. The button is DISABLED until the chat has a saved
 * conversation, so loading `/chat/{id}` (which hydrates `Stores.Chat.conversation`)
 * is what enables it.
 */
export async function openScheduleDialog(
  page: Page,
  baseURL: string,
  conversationId: string,
): Promise<void> {
  await page.goto(`${baseURL}/chat/${conversationId}`)
  await page.waitForLoadState('load')
  // The composer renders on a loaded conversation.
  await page.waitForSelector('textarea[placeholder*="Type your message"]', {
    timeout: 30000,
  })

  const button = byTestId(page, 'chat-schedule-loop-button')
  await expect(button).toBeVisible({ timeout: 15000 })
  // Enabled only once the conversation is hydrated into the pane's chat store.
  await expect(button).toBeEnabled({ timeout: 15000 })
  await button.click()

  await expect(byTestId(page, 'schedule-loop-form')).toBeVisible({ timeout: 15000 })
}

/**
 * Pick a value in a kit Select by its trigger testid (open → click the derived
 * `${testid}-opt-${value}` option). Mirrors `14-scheduler/helpers.ts::pickSelect`.
 */
export async function pickSelectValue(
  page: Page,
  triggerTestid: string,
  value: string,
): Promise<void> {
  await byTestId(page, triggerTestid).click()
  const opt = byTestId(page, `${triggerTestid}-opt-${value}`)
  await opt.waitFor({ state: 'visible', timeout: 10000 })
  await opt.click()
}

/**
 * Reliably switch a kit Segmented (base-ui Tabs) to `optTestid`. base-ui Tabs'
 * raised active-pill layer fools Playwright's pointer hit-test, so a plain
 * `.click()` on an inactive segment is a no-op; a lone `force` click doesn't
 * always activate the nested `schedule-kind` control either. Hovering first
 * (to establish pointer state) then force-clicking is the sequence empirically
 * proven to flip the segment — the caller still asserts the resulting UI.
 */
export async function switchSegment(page: Page, optTestid: string): Promise<void> {
  const opt = byTestId(page, optTestid)
  await opt.hover().catch(() => {})
  await opt.click().catch(() => {})
  await opt.click({ force: true })
}

/** Fetch the acting admin's scheduled tasks bound to a conversation (owner-scoped). */
export async function getTasksForConversation(
  page: Page,
  apiURL: string,
  adminToken: string,
  conversationId: string,
): Promise<Array<Record<string, unknown>>> {
  const res = await page.request.get(
    `${apiURL}/api/scheduled-tasks?conversation_id=${conversationId}`,
    { headers: { Authorization: `Bearer ${adminToken}` } },
  )
  if (!res.ok()) {
    throw new Error(`list tasks failed: ${res.status()} ${await res.text()}`)
  }
  return (await res.json()) as Array<Record<string, unknown>>
}
