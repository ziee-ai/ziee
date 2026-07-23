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
 * TEST-14 — the friendly AGENT step form (ITEM-9). Domain language over tool
 * jargon: a "What should the assistant do?" instructions box, a capability
 * MultiSelect, a named EFFORT segmented (Quick / Balanced / Thorough), an OUTPUT
 * segmented, a plain-English read-back sentence, and an Advanced disclosure
 * carrying the system directive. Configure + Save → a runnable agent workflow.
 * Authoring only (no run). No API mocking.
 */

test.describe('Workflows — builder agent step (friendly form)', () => {
  test('the agent form renders its friendly controls, read-back, advanced → saves', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const wfName = `e2e-builder-agent-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await openNewBuilder(page, baseURL)

    const agentId = await addStep(page, 'agent', 1) // agent_1
    const cfg = byTestId(page, 'wf-builder-step-config')

    // The friendly instructions box, labelled in plain language.
    await expect(cfg).toContainText('What should the assistant do?')
    const prompt = byTestId(page, 'wf-builder-agent-prompt')
    await expect(prompt).toBeVisible()

    // A capability MultiSelect.
    await expect(byTestId(page, 'wf-builder-agent-servers')).toBeVisible()

    // The effort Segmented offers Quick / Balanced / Thorough.
    await expect(byTestId(page, 'wf-builder-agent-effort-opt-quick')).toContainText(
      'Quick',
    )
    await expect(
      byTestId(page, 'wf-builder-agent-effort-opt-balanced'),
    ).toContainText('Balanced')
    await expect(
      byTestId(page, 'wf-builder-agent-effort-opt-thorough'),
    ).toContainText('Thorough')
    // Default is Balanced (30 steps → the balanced stop).
    await expect(
      byTestId(page, 'wf-builder-agent-effort-opt-balanced'),
    ).toHaveAttribute('data-state', 'on')

    // The output Segmented (Text / Structured).
    await expect(byTestId(page, 'wf-builder-agent-output-opt-text')).toBeVisible()
    await expect(byTestId(page, 'wf-builder-agent-output-opt-json')).toBeVisible()

    // Configure the task, bump effort to Thorough, ask for a structured result.
    await prompt.fill('Find the three most-cited papers and summarise them')
    await byTestId(page, 'wf-builder-agent-effort-opt-thorough').click()
    await expect(
      byTestId(page, 'wf-builder-agent-effort-opt-thorough'),
    ).toHaveAttribute('data-state', 'on')
    await byTestId(page, 'wf-builder-agent-output-opt-json').click()

    // The plain-English read-back reflects the configuration.
    const readback = byTestId(page, 'wf-builder-agent-readback')
    await expect(readback).toContainText(
      'Find the three most-cited papers and summarise them',
    )
    await expect(readback).toContainText('60 steps') // Thorough
    await expect(readback).toContainText('structured result') // Structured output

    // Advanced disclosure: the system directive is hidden until expanded.
    await expect(byTestId(page, 'wf-builder-agent-system')).toHaveCount(0)
    await byTestId(page, 'wf-builder-agent-advanced')
      .getByText('Advanced', { exact: true })
      .click()
    const system = byTestId(page, 'wf-builder-agent-system')
    await expect(system).toBeVisible()
    await system.fill('You are a meticulous research assistant.')

    // Save → a runnable agent workflow is created.
    await byTestId(page, 'wf-builder-name').fill(wfName)
    await waitBuilderValid(page)
    await saveBuilder(page)
    await expect(page).toHaveURL(/\/settings\/workflows\/[0-9a-f-]+\/edit$/, {
      timeout: 15000,
    })

    // The created workflow carries an agent step (proves it persisted as an
    // agent workflow, not merely that a row was written).
    await page.goto(`${baseURL}/settings/workflows`, {
      waitUntil: 'domcontentloaded',
    })
    await byTestId(page, 'wf-list-page-title').first().waitFor({ timeout: 15000 })
    const card = page
      .locator('[data-testid^="wf-list-card-"]')
      .filter({ hasText: wfName })
      .first()
    await expect(card).toBeVisible({ timeout: 15000 })
    await card.click()
    await expect(byTestId(page, 'wf-detail-drawer')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'wf-detail-step-kind-tag-0')).toContainText(
      'agent',
    )
    // Reference the deterministic agent id so the linter keeps it meaningful.
    expect(agentId).toBe('agent_1')
  })
})
