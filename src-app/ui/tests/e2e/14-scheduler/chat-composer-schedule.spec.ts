import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { byTestId } from '../testid'
import {
  openScheduleDialog,
  pickSelectValue,
  seedModelAndConversation,
  getTasksForConversation,
} from './chat-schedule-helpers'

/**
 * TEST-81 (ITEM-18) — the in-chat "Schedule or loop this chat" composer button.
 *
 * asserts (TESTS.md): toolbar Schedule/Loop button for a `scheduler::use` user;
 * opens the merged dialog; saving a prompt task creates a row bound to the current
 * conversation; a no-perm user sees no button.
 *
 * Driven against the REAL backend (not mocked) because the load-bearing claim is
 * the actual bind: the created `scheduled_task` carries `bound_conversation_id =
 * <this conversation>` — a fact only the real `create_task` handler can produce
 * (it validates both the model access and the conversation ownership).
 */
test.describe('In-chat schedule composer button (ITEM-18)', () => {
  test('admin: button opens the dialog and creates a prompt task bound to the current conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Positive control — an admin holds `scheduler::use` (Users group grant + `*`).
    await loginAsAdmin(page, baseURL)
    const seed = await seedModelAndConversation(page, apiURL)

    // The toolbar button opens the merged dialog (proven inside openScheduleDialog:
    // button visible + enabled once the conversation is hydrated, then the form).
    await openScheduleDialog(page, baseURL, seed.conversationId)

    // Default mode is "Schedule" (recurring). Fill the required message + model.
    await byTestId(page, 'schedule-loop-prompt').fill(
      'Search PubMed for new CRISPR papers and summarise them',
    )
    await pickSelectValue(page, 'task-form-model', seed.modelId)

    // Save → a real POST /api/scheduled-tasks.
    const [resp] = await Promise.all([
      page.waitForResponse(
        r =>
          /\/api\/scheduled-tasks$/.test(r.url()) &&
          r.request().method() === 'POST',
        { timeout: 20000 },
      ),
      byTestId(page, 'schedule-loop-submit').click(),
    ])
    expect(resp.ok()).toBeTruthy()

    // The dialog closes on success.
    await expect(byTestId(page, 'schedule-loop-form')).toHaveCount(0, {
      timeout: 15000,
    })

    // The row exists AND is bound to THIS conversation (owner-scoped ?conversation_id
    // filter returns ONLY the bound tasks — TEST-91's REST contract).
    const tasks = await getTasksForConversation(
      page,
      apiURL,
      seed.adminToken,
      seed.conversationId,
    )
    expect(tasks.length).toBe(1)
    expect(tasks[0].bound_conversation_id).toBe(seed.conversationId)
    expect(tasks[0].target_kind).toBe('prompt')
    expect(tasks[0].model_id).toBe(seed.modelId)
  })

  test('a user without scheduler::use sees no schedule button (gated at the composer)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // A realistic non-scheduler user: full chat perms (so the composer renders —
    // a non-vacuous negative) but NOT `scheduler::use`. loginWithPerms strips the
    // default Users group (which bundles scheduler::use) and grants only these.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [
        'chat::read',
        'chat::create',
        'conversations::create',
        'conversations::read',
        'conversations::edit',
        'messages::create',
        'messages::read',
      ],
      'sched-noperm',
    )

    // Open a conversation this user owns — the SAME surface (ConversationPage) where
    // the admin's button appears — so the button's absence is a real permission gate,
    // not a missing composer.
    const token = await getCurrentUserToken(page)
    const convRes = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'No-scheduler-perm chat' },
    })
    expect(convRes.ok()).toBe(true)
    const convId = (await convRes.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForLoadState('load')
    // The composer DID render (non-vacuous) …
    await page.waitForSelector('textarea[placeholder*="Type your message"]', {
      timeout: 30000,
    })
    // … but the scheduler-gated button is absent (usePermission(SchedulerUse) → null).
    await expect(byTestId(page, 'chat-schedule-loop-button')).toHaveCount(0)
  })
})
