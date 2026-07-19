import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  addStep,
  openNewBuilder,
  saveBuilder,
  waitBuilderValid,
} from './helpers/builder-helpers'

/**
 * TEST-10 — the workflow VISUAL BUILDER create flow (ITEM-7), end-to-end
 * against the real backend (no API mocking).
 *
 *   /settings/workflows → "New workflow" opens the builder → add two steps via
 *   the kind picker → reorder them (accessible up/down) → edit a step's title →
 *   name + Save (POST /api/workflows) → the new workflow appears in the list →
 *   its detail drawer shows the steps.
 */

test.describe('Workflows — builder create', () => {
  test('build, reorder, retitle, save → workflow appears with its steps', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const wfName = `e2e-builder-create-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // "New workflow" opens the builder (create mode).
    await openNewBuilder(page, baseURL)

    // Add two LLM steps via the real kind picker; each gets a deterministic id.
    const first = await addStep(page, 'llm', 1) // llm_1
    const second = await addStep(page, 'llm', 2) // llm_2

    // Configure step 1: a label + a prompt (a valid llm step).
    await byTestId(page, `wf-builder-step-row-${first}`).click()
    await byTestId(page, 'wf-builder-step-description').fill('Gather sources')
    await byTestId(page, 'wf-builder-llm-prompt').fill(
      'Find sources on the topic.',
    )

    // Configure step 2 similarly.
    await byTestId(page, `wf-builder-step-row-${second}`).click()
    await byTestId(page, 'wf-builder-step-description').fill('Synthesize answer')
    await byTestId(page, 'wf-builder-llm-prompt').fill(
      'Synthesize from the gathered sources.',
    )

    // Initial order is [llm_1, llm_2].
    const rows = page.locator('[data-testid^="wf-builder-step-row-"]')
    await expect(rows).toHaveCount(2)
    await expect(rows.nth(0)).toHaveAttribute(
      'data-testid',
      `wf-builder-step-row-${first}`,
    )

    // Reorder: move step 2 UP (accessible button). Order becomes [llm_2, llm_1].
    await byTestId(page, `wf-builder-step-up-${second}`).click()
    await expect(rows.nth(0)).toHaveAttribute(
      'data-testid',
      `wf-builder-step-row-${second}`,
    )
    await expect(rows.nth(1)).toHaveAttribute(
      'data-testid',
      `wf-builder-step-row-${first}`,
    )

    // Edit a title: retitle the now-first step (llm_2) and see it reflected in
    // the step row (rows show `description || id`).
    await byTestId(page, `wf-builder-step-row-${second}`).click()
    await byTestId(page, 'wf-builder-step-description').fill('Synthesis (edited)')
    await expect(
      byTestId(page, `wf-builder-step-row-${second}`),
    ).toContainText('Synthesis (edited)')

    // Name it, wait for the live validation to clear, then Save.
    await byTestId(page, 'wf-builder-name').fill(wfName)
    await waitBuilderValid(page)
    await saveBuilder(page)

    // Create-mode Save success message + redirect to the edit route (in place).
    await expect(page).toHaveURL(/\/settings\/workflows\/[0-9a-f-]+\/edit$/, {
      timeout: 15000,
    })

    // Back to the list — the new workflow is present as a card.
    await page.goto(`${baseURL}/settings/workflows`, {
      waitUntil: 'domcontentloaded',
    })
    await byTestId(page, 'wf-list-page-title').first().waitFor({ timeout: 15000 })
    const card = page
      .locator('[data-testid^="wf-list-card-"]')
      .filter({ hasText: wfName })
      .first()
    await expect(card).toBeVisible({ timeout: 15000 })

    // Open the detail drawer — it lists the two steps (retitled) with kind tags.
    await card.click()
    const drawer = byTestId(page, 'wf-detail-drawer')
    await expect(drawer).toBeVisible({ timeout: 15000 })
    await expect(drawer).toContainText('Synthesis (edited)')
    await expect(drawer).toContainText('Gather sources')
    // The step kind tags render (llm).
    await expect(byTestId(page, 'wf-detail-step-kind-tag-0')).toContainText('llm')
  })
})
