import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToWorkflowsPage } from './helpers/workflow-helpers'

// E2E for ImportWorkflowDialog (audit id 877d258b5d5e — zero coverage):
// the file picker, the Validate button + inline validation-result rendering
// (valid + invalid), and the non-yaml advisory branch. The server-side
// /api/workflows/validate boundary is mocked (this spec asserts the UI
// contract — validate → result Alert — not the validator internals).

type ValEntry = { code: string; location?: string; message: string }
type ValResponse = {
  valid: boolean
  steps: number
  est_max_calls: number
  est_max_tokens: number
  errors: ValEntry[]
  warnings: ValEntry[]
}

async function mockValidate(page: Page, resp: ValResponse) {
  await page.route(/\/api\/workflows\/validate$/, async route => {
    if (route.request().method() === 'POST') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(resp),
      })
    }
    return route.continue()
  })
}

async function openImportDialog(page: Page) {
  await page.getByRole('button', { name: /import/i }).click()
  await expect(
    page.getByRole('dialog').getByText('Import Workflow'),
  ).toBeVisible()
}

async function dropFile(page: Page, name: string, contents: string) {
  // The antd Dragger renders a hidden <input type=file>; setInputFiles drives
  // the real onChange path (beforeUpload returns false, so no network upload).
  await page
    .locator('input[type="file"]')
    .setInputFiles({
      name,
      mimeType: 'application/x-yaml',
      buffer: Buffer.from(contents, 'utf8'),
    })
}

test.describe('Workflows — Import dialog', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToWorkflowsPage(page, testInfra.baseURL)
  })

  test('validating a valid workflow.yaml shows the success result', async ({
    page,
  }) => {
    await mockValidate(page, {
      valid: true,
      steps: 4,
      est_max_calls: 12,
      est_max_tokens: 100000,
      errors: [],
      warnings: [],
    })
    await openImportDialog(page)
    await dropFile(page, 'workflow.yaml', 'name: demo\nsteps: []\n')
    await page.getByRole('button', { name: 'Validate' }).click()

    // Success Alert renders the step + call counts.
    await expect(
      page.getByText(/Valid workflow — 4 steps, up to 12 calls/i),
    ).toBeVisible({ timeout: 5000 })
  })

  test('validating an invalid workflow.yaml surfaces the error messages', async ({
    page,
  }) => {
    await mockValidate(page, {
      valid: false,
      steps: 0,
      est_max_calls: 0,
      est_max_tokens: 0,
      errors: [{ code: 'E_SCHEMA', location: 'steps[0]', message: 'missing run' }],
      warnings: [],
    })
    await openImportDialog(page)
    await dropFile(page, 'workflow.yaml', 'name: broken\n')
    await page.getByRole('button', { name: 'Validate' }).click()

    await expect(page.getByText('Validation failed')).toBeVisible({
      timeout: 5000,
    })
    await expect(page.getByText(/steps\[0\]: missing run/)).toBeVisible()
  })

  test('validating a non-yaml bundle shows the advisory instead of calling validate', async ({
    page,
  }) => {
    let validateCalled = false
    await page.route(/\/api\/workflows\/validate$/, async route => {
      validateCalled = true
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: '{}',
      })
    })
    await openImportDialog(page)
    await dropFile(page, 'bundle.tar.gz', 'binary-bundle-bytes')
    await page.getByRole('button', { name: 'Validate' }).click()

    // The dialog advises the user to drop a workflow.yaml; no validate call.
    await expect(
      page.getByText(/Validation reads workflow\.yaml/i),
    ).toBeVisible({ timeout: 5000 })
    expect(validateCalled).toBe(false)
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
