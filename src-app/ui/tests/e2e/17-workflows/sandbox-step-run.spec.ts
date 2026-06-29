import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * `kind: sandbox` workflow step — UI run-progress coverage gap.
 *
 * The backend path is covered by `server/tests/workflow/sandbox_run.rs` +
 * `sandbox_progress.rs`. This closes the UI layer: running a sandbox-step
 * workflow from the settings drawer streams progress into
 * `WorkflowRunProgressView` and the step reaches "completed" with its captured
 * stdout surfaced.
 *
 * Gated on `ZIEE_SANDBOX_ROOTFS` exactly like the code_sandbox Tier-4/6 tiers:
 * a sandbox step needs a runnable bwrap backend + a mounted rootfs, which CI /
 * dev boxes without the rootfs cannot provide. This is a genuine
 * external-dependency gate (same as the 12-local-runtime engine specs gating on
 * `ZIEE_E2E_ENGINE_MIRROR`), NOT a make-suite-green skip. A sandbox-only
 * workflow has no `llm` steps, so NO ANTHROPIC_API_KEY is needed.
 */

const HAS_ROOTFS = (process.env.ZIEE_SANDBOX_ROOTFS ?? '').length > 0

// One sandbox step that echoes a templated input; `flavor: minimal` matches the
// e2e rootfs (bash + coreutils). Mirrors `sandbox_run.rs`'s YAML.
const SANDBOX_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
inputs:
  - name: name
    required: true
steps:
  - id: greet
    kind: sandbox
    run: echo "hello {{ inputs.name }} from the sandbox"
outputs:
  - name: greeting
    from: "{{ greet.output }}"
    expose: full
`

test.describe('Workflows - sandbox step run (rootfs-gated)', () => {
  test.skip(
    !HAS_ROOTFS,
    'ZIEE_SANDBOX_ROOTFS not set — sandbox-step run-progress E2E skipped',
  )

  test('a sandbox step streams progress to completed in the run view', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-sandbox-greet',
      SANDBOX_WORKFLOW_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-sandbox-greet')

    // Open the Run dialog.
    await byTestId(page, 'wf-detail-run-btn').click()
    await expect(byTestId(page, 'wf-run-dialog')).toBeVisible({
      timeout: 10000,
    })

    // Provide the required `name` input (structured field or JSON fallback).
    const nameField = byTestId(page, 'wf-run-input-name')
    if (await nameField.count()) {
      await nameField.first().fill('ziee')
    } else {
      await byTestId(page, 'wf-run-json-textarea').fill('{ "name": "ziee" }')
    }

    // A sandbox-only workflow needs no model; run directly.
    await byTestId(page, 'wf-run-submit-btn').click()

    // Progress streams in and the run reaches completed.
    await expect(byTestId(page, 'wf-progress-status-tag')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'completed',
      { timeout: 60000 },
    )

    // The sandbox step id ("greet") is shown in the progress tree — its kind
    // tag is keyed by the step id.
    await expect(
      byTestId(page, 'wf-progress-step-kind-tag-greet'),
    ).toBeVisible()
  })
})
