import { expect, type Page } from '@playwright/test'
import { byTestId } from '../../testid'
import type { StepKind } from '../../../../src/modules/workflow/components/builder/stepForms'

/**
 * Helpers for the workflow VISUAL BUILDER e2e specs (TEST-10..16/20).
 *
 * These drive the REAL builder surface end-to-end (no API mocking): the create
 * route (`/settings/workflows/builder`) and edit route
 * (`/settings/workflows/:id/edit`) both render `WorkflowBuilderPage`, backed by
 * the per-instance `WorkflowBuilder.store` which live-validates every edit
 * against the real `POST /api/workflows/validate-def` and persists via
 * `POST/PUT /api/workflows`.
 *
 * Selector map (from the builder components):
 *   page title        wf-builder-page-title       ("New workflow" / "Edit workflow")
 *   name (create)     wf-builder-name
 *   add-step btn      wf-builder-add-step-btn      → Dropdown wf-builder-add-step-menu
 *   add-step item     wf-builder-add-step-menu-item-<kind>
 *   step row          wf-builder-step-row-<stepId> (stepId = `<kind>_<n>`)
 *   step kind tag     wf-builder-step-kind-<stepId>
 *   move up/down      wf-builder-step-up-<stepId> / wf-builder-step-down-<stepId>
 *   step label field  wf-builder-step-description
 *   validation ok     wf-builder-valid            (present ⇔ 0 blocking errors)
 *   save              wf-builder-save-btn
 */

/** Navigate to the list page and click "New workflow" → land on the builder. */
export async function openNewBuilder(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/workflows`, {
    waitUntil: 'domcontentloaded',
  })
  await byTestId(page, 'wf-list-new-btn').waitFor({ timeout: 20000 })
  await byTestId(page, 'wf-list-new-btn').click()
  await expect(byTestId(page, 'wf-builder-page-title')).toBeVisible({
    timeout: 15000,
  })
  await expect(byTestId(page, 'wf-builder-page-title')).toContainText(
    'New workflow',
  )
}

/**
 * Add a step of `kind` via the real kind picker. The Dropdown items carry
 * `wf-builder-add-step-menu-item-<kind>`; clicking appends the step + selects
 * it (the config panel renders it). Returns the deterministic step id the store
 * assigns (`<kind>_<n>`) so callers can target the new row/config.
 */
export async function addStep(
  page: Page,
  kind: StepKind,
  nth: number,
): Promise<string> {
  await byTestId(page, 'wf-builder-add-step-btn').click()
  const item = byTestId(page, `wf-builder-add-step-menu-item-${kind}`)
  await item.waitFor({ state: 'visible', timeout: 5000 })
  await item.click()
  const stepId = `${kind}_${nth}`
  // The new row appears and the config panel switches to it.
  await expect(byTestId(page, `wf-builder-step-row-${stepId}`)).toBeVisible({
    timeout: 5000,
  })
  await expect(byTestId(page, 'wf-builder-step-config')).toBeVisible()
  return stepId
}

/** The add-step kind picker lists these 6 kinds (STEP_KINDS). */
export const ALL_STEP_KINDS: StepKind[] = [
  'agent',
  'llm',
  'llm_map',
  'sandbox',
  'elicit',
  'tool',
]

/**
 * Wait for the live-validation feed to report NO blocking errors. Save is
 * disabled while `validation.errors` is non-empty (the page gate), so specs
 * that Save must reach this state first. The store debounces validate ~400ms
 * then POSTs, so give it a generous budget.
 */
export async function waitBuilderValid(page: Page) {
  await expect(byTestId(page, 'wf-builder-valid')).toBeVisible({
    timeout: 20000,
  })
}

/** Save and wait for the Save to land (button leaves its loading state). */
export async function saveBuilder(page: Page) {
  const save = byTestId(page, 'wf-builder-save-btn')
  await expect(save).toBeEnabled({ timeout: 20000 })
  await save.click()
}
