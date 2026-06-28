import { test, expect } from '../../fixtures/test-context'
import type { APIRequestContext } from '@playwright/test'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — conversation-switch race in the summarization read-model.
 *
 * `ConversationSummarization.store.ts` drops stale results when the active
 * conversation id changes while a `loadForConversation` is still in flight
 * (and `SummarizationStatusPill` re-triggers the load on conversation switch).
 * The hazard: opening conversation A then quickly B could let A's slower
 * response overwrite B's read-model, showing A's summary under B.
 *
 * This guards the surface: seed TWO conversations with DISTINCT summaries, open
 * A, then immediately switch to B, and assert B's in-thread boundary divider
 * shows B's summary text — never A's. No LLM (summaries are seeded via SQL,
 * mirroring `in-thread-marker.spec.ts`).
 */

type Sql = (q: string, params?: unknown[]) => Promise<{ rows: unknown[] }>

async function seedConversationWithSummary(
  request: APIRequestContext,
  apiURL: string,
  token: string,
  sql: Sql,
  title: string,
  summaryText: string,
): Promise<{ convId: string; anchorId: string }> {
  const created = await request.post(`${apiURL}/api/conversations`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { title },
  })
  expect(created.ok()).toBe(true)
  const conv = await created.json()
  const convId = conv.id as string
  const branchId = conv.active_branch_id as string

  const messageIds: string[] = []
  for (let i = 0; i < 5; i++) {
    const role = i % 2 === 0 ? 'user' : 'assistant'
    const text = `${title} turn ${i}.`
    const inserted = await sql(
      `INSERT INTO messages (id, role, originated_from_id, edit_count, created_at)
       VALUES (gen_random_uuid(), $1, gen_random_uuid(), 0, NOW() + ($2::int * INTERVAL '1 second'))
       RETURNING id`,
      [role, i],
    )
    const msgId = (inserted.rows[0] as { id: string }).id
    messageIds.push(msgId)
    await sql(
      `INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
       VALUES ($1, $2, false, NOW() + ($3::int * INTERVAL '1 second'))`,
      [branchId, msgId, i],
    )
    await sql(
      `INSERT INTO message_contents (message_id, content_type, content, sequence_order)
       VALUES ($1, 'text', $2::jsonb, 0)`,
      [msgId, JSON.stringify({ type: 'text', text })],
    )
  }
  const anchorId = messageIds[2]!
  await sql(
    `INSERT INTO conversation_summaries
       (branch_id, summary_text, summarized_up_to_id, message_count, model_used)
     VALUES ($1, $2, $3, 3, 'test-model')`,
    [branchId, summaryText, anchorId],
  )
  return { convId, anchorId }
}

test.describe('Summarization — conversation-switch race', () => {
  test('switching from A to B shows B\'s summary, never A\'s', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const summaryA = `AAA summary ${Date.now()} — alpha conversation only.`
    const summaryB = `BBB summary ${Date.now()} — beta conversation only.`

    const a = await seedConversationWithSummary(
      request,
      apiURL,
      token,
      sql,
      'race-conv-A',
      summaryA,
    )
    const b = await seedConversationWithSummary(
      request,
      apiURL,
      token,
      sql,
      'race-conv-B',
      summaryB,
    )

    // Open A, then immediately switch to B (client-side nav) to race the loads.
    await page.goto(`${baseURL}/chat/${a.convId}`)
    await page.goto(`${baseURL}/chat/${b.convId}`)

    // B's boundary divider must reflect B's summary.
    const dividerB = page
      .locator(`[data-message-id="${b.anchorId}"]`)
      .getByText(/condensed into a summary/i)
    await expect(dividerB).toBeVisible({ timeout: 30000 })
    await dividerB.click()
    await expect(page.getByText(summaryB)).toBeVisible({ timeout: 5000 })

    // A's summary must NOT have leaked into B's view.
    await expect(page.getByText(summaryA)).toHaveCount(0)
  })
})
