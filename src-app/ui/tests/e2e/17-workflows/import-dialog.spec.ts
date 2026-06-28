import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToWorkflowsSettingsPage } from './helpers/workflow-helpers'

/**
 * Import Workflow dialog UX (no prior E2E). Opens the dialog from the workflows
 * settings page, drops a workflow.yaml into the Dragger, and drives the
 * server-backed Validate path: a well-formed workflow validates green (step
 * count surfaced), and a malformed one surfaces "Validation failed". Only the
 * validate HTTP boundary is real — the yaml content is the behavior under test.
 */

const VALID_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    description: "subject"
    required: true
steps:
  - id: summarize
    kind: llm
    message: "Summarizing {{ inputs.topic }}"
    prompt: |
      One sentence about "{{ inputs.topic }}".
outputs:
  - name: summary
    from: "{{ summarize.output }}"
    expose: full
`

const INVALID_YAML = `this is: not a workflow
random: [unclosed
`

async function dropYaml(page: import('@playwright/test').Page, name: string, body: string) {
  await page.locator('.ant-modal input[type="file"]').setInputFiles({
    name,
    mimeType: 'text/yaml',
    buffer: Buffer.from(body),
  })
}

test.describe('Workflows - Import dialog', () => {
  test('validate surfaces a green result for valid yaml and an error for invalid', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToWorkflowsSettingsPage(page, baseURL)

    // Open the Import dialog.
    await page.getByRole('button', { name: 'Import' }).click()
    const dialog = page.getByRole('dialog', { name: 'Import Workflow' })
    await expect(dialog).toBeVisible()
    await expect(
      dialog.getByText(/Drop a workflow bundle .* or workflow\.yaml/),
    ).toBeVisible()

    // Drop a VALID workflow.yaml → Validate → green "Valid workflow" alert with
    // the parsed step count (server validated the definition).
    await dropYaml(page, 'workflow.yaml', VALID_YAML)
    await dialog.getByRole('button', { name: 'Validate' }).click()
    await expect(dialog.getByText(/Valid workflow — 1 steps/)).toBeVisible({
      timeout: 15000,
    })

    // Replace with a MALFORMED workflow.yaml → Validate → "Validation failed".
    await dropYaml(page, 'workflow.yaml', INVALID_YAML)
    await dialog.getByRole('button', { name: 'Validate' }).click()
    await expect(dialog.getByText('Validation failed')).toBeVisible({
      timeout: 15000,
    })
  })
})
