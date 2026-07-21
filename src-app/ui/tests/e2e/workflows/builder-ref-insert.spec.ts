import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import { addStep, openNewBuilder } from './helpers/builder-helpers'

/**
 * TEST-16 — the ref-insert menu (ITEM-10) on a step's prompt field lists the
 * workflow's inputs + every PRIOR step's output, and inserting one appends the
 * correct reference token (`{{ inputs.x }}` / `{{ step.output }}`). No mocking —
 * pure client authoring surface driven through the real builder.
 */

test.describe('Workflows — builder ref-insert menu', () => {
  test('lists inputs + prior-step outputs and inserts the right token', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await openNewBuilder(page, baseURL)

    // Declare an input `topic`.
    await byTestId(page, 'wf-builder-input-add').click()
    await byTestId(page, 'wf-builder-input-name-0').fill('topic')

    // Two llm steps; edit the SECOND so the first is a referenceable prior step.
    await addStep(page, 'llm', 1) // llm_1
    const second = await addStep(page, 'llm', 2) // llm_2
    await byTestId(page, `wf-builder-step-row-${second}`).click()

    const promptField = byTestId(page, 'wf-builder-llm-prompt')
    await expect(promptField).toHaveValue('')

    // Open the ref-insert menu on the prompt field.
    await byTestId(page, 'wf-builder-llm-prompt-ref-trigger').click()
    const menu = byTestId(page, 'wf-builder-llm-prompt-ref')
    await expect(menu).toBeVisible()
    // It groups the input + the prior step.
    await expect(menu).toContainText('Inputs')
    await expect(menu).toContainText('topic')
    await expect(menu).toContainText('Previous steps')
    await expect(menu).toContainText('llm_1')

    // Insert the input reference → the exact token lands in the field.
    await page.getByRole('menuitem', { name: /topic/ }).click()
    await expect(promptField).toHaveValue('{{ inputs.topic }}')

    // Insert the prior step's output → the step-output token is appended.
    await byTestId(page, 'wf-builder-llm-prompt-ref-trigger').click()
    await expect(byTestId(page, 'wf-builder-llm-prompt-ref')).toBeVisible()
    await page.getByRole('menuitem', { name: /llm_1/ }).click()
    await expect(promptField).toHaveValue('{{ inputs.topic }}{{ llm_1.output }}')
  })
})
