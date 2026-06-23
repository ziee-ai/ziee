import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedLiteratureResult, sampleResult } from './fixtures/mock-literature-result'

/**
 * CHAT-interface screening RESUME: when the literature panel is opened from a
 * SUSPENDED sr-review screening gate, the human screens and "Submit screening &
 * continue" derives `included_ids` from the Include rows and POSTs
 * `Workflow.submitElicit` to resume the run.
 *
 * Determinism / "test what works": there's no chat-native way to reach the gate
 * (the run-progress view isn't rendered in chat), so we exercise the REAL panel
 * submit wiring by seeding the panel into the gate state — open the read-only
 * panel from the tool-result card, then inject `runId`/`elicitationId` into the
 * persisted panel tab (localStorage) + reload so it rehydrates as a gated
 * session. The submit endpoint is route-mocked to capture the request body. No
 * live LLM/network.
 */

const RUN_ID = '11111111-1111-4111-8111-111111111111'
const ELICITATION_ID = '22222222-2222-4222-8222-222222222222'

test.describe('Literature screening — resume a suspended run from the chat panel', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(
      () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('screen in the panel → "Submit screening & continue" → submitElicit with the included set', async ({
    page,
    testInfra,
  }) => {
    // Capture the submitElicit POST body (and 200 it) so the panel's resume call
    // is asserted without a real run.
    const submits: Array<{ url: string; body: unknown }> = []
    await page.route(`**/api/workflow-runs/${RUN_ID}/elicit/${ELICITATION_ID}`, async route => {
      submits.push({ url: route.request().url(), body: route.request().postDataJSON() })
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ elicitation_id: ELICITATION_ID, run_id: RUN_ID, status: 'delivered' }),
      })
    })

    // Open the read-only screening panel from the inline tool-result card.
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())
    const openBtn = page.getByRole('button', { name: /Open in screening/ })
    await expect(openBtn).toBeVisible({ timeout: 10000 })
    await openBtn.click()
    await expect(page.getByRole('heading', { name: 'Screening' })).toBeVisible({ timeout: 10000 })

    // Inject the gate handle into the persisted panel tab, then reload so the
    // panel rehydrates as a GATED session (runId + elicitationId set).
    await page.waitForFunction(() => {
      const raw = localStorage.getItem('ziee-right-panel-tabs-v2')
      if (!raw) return false
      return Object.values(JSON.parse(raw)).some((s: any) =>
        s?.tabs?.some((t: any) => t.type === 'literature'),
      )
    })
    await page.evaluate(
      ({ runId, eid }) => {
        const raw = localStorage.getItem('ziee-right-panel-tabs-v2')!
        const all = JSON.parse(raw)
        for (const k of Object.keys(all)) {
          for (const t of all[k].tabs ?? []) {
            if (t.type === 'literature') {
              t.data.runId = runId
              t.data.elicitationId = eid
            }
          }
        }
        localStorage.setItem('ziee-right-panel-tabs-v2', JSON.stringify(all))
      },
      { runId: RUN_ID, eid: ELICITATION_ID },
    )
    await page.reload()
    await page.waitForLoadState('load')

    // The resume affordance appears only for a gated session.
    const submitBtn = page.getByRole('button', { name: /Submit screening/ })
    await expect(submitBtn).toBeVisible({ timeout: 15000 })

    // Include both candidates (bulk), then submit to resume the run.
    await page.getByRole('checkbox', { name: /Select all|selected/ }).click()
    await page.getByRole('button', { name: 'Include', exact: true }).click()
    await expect(page.getByText('Included: 2')).toBeVisible()
    await submitBtn.click()

    // The panel POSTed submitElicit with the derived included_ids (the two
    // sample DOIs) + approved:true.
    await expect.poll(() => submits.length, { timeout: 10000 }).toBeGreaterThan(0)
    const response = (submits[0].body as { response?: Record<string, unknown> }).response ?? {}
    expect(response.approved).toBe(true)
    const included = (response.included_ids as string[]) ?? []
    expect(included).toEqual(expect.arrayContaining(['10.1/aaa', '10.1/bbb']))
  })
})
