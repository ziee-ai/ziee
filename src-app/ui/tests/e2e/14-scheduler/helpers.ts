import type { Page } from '@playwright/test'

import { byTestId } from '../testid'

/**
 * Shared seed + picker-driving helpers for the 14-scheduler e2e suite. The
 * scheduler drawer's target pickers (Assistant / Model / Workflow / allowed
 * tools) read their option lists from the AssistantPicker / ModelPicker /
 * Workflow / McpServer stores, which fetch these list endpoints. We mock them at
 * the HTTP boundary so the pickers have deterministic options — replacing the
 * old raw-UUID text inputs the specs used to `fill()`.
 */

export const MODEL_ID = '11111111-1111-1111-1111-111111111111'
export const MODEL_LABEL = 'Test Model'
export const ASSISTANT_ID = '33333333-3333-3333-3333-333333333333'
export const ASSISTANT_NAME = 'Research Bot'
export const WORKFLOW_ID = '99999999-9999-9999-9999-999999999999'
export const WORKFLOW_NAME = 'Weekly Digest Flow'
export const WORKFLOW_NO_INPUT_ID = 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa'
export const WORKFLOW_NO_INPUT_NAME = 'Bare Flow'
export const SERVER_ID = '88888888-8888-8888-8888-888888888888'
export const SERVER_NAME = 'Files Server'

const defaultProviders = [
  {
    id: 'prov-1',
    name: 'Test Provider',
    llm_models: [
      {
        id: MODEL_ID,
        name: 'test-model',
        display_name: MODEL_LABEL,
        enabled: true,
      },
    ],
  },
]

const defaultAssistants = [{ id: ASSISTANT_ID, name: ASSISTANT_NAME }]

/** A workflow whose compiled IR declares a typed input (drives typed-field mode). */
export const workflowWithInputs = {
  id: WORKFLOW_ID,
  name: WORKFLOW_NAME,
  display_name: WORKFLOW_NAME,
  compiled_ir_json: {
    inputs: [{ name: 'topic', description: 'Search topic', required: true }],
    steps: [],
  },
}

/** A workflow with no declared inputs (drives the JSON-fallback editor). */
export const workflowNoInputs = {
  id: WORKFLOW_NO_INPUT_ID,
  name: WORKFLOW_NO_INPUT_NAME,
  display_name: WORKFLOW_NO_INPUT_NAME,
  compiled_ir_json: { inputs: [], steps: [] },
}

interface PickerSeed {
  providers?: unknown[]
  assistants?: unknown[]
  workflows?: unknown[]
  servers?: unknown[]
}

/** Mock the four picker list endpoints. Call BEFORE login so every fetch (boot +
 *  drawer-open) is served deterministically. */
export async function mockPickerEndpoints(
  page: Page,
  seed: PickerSeed = {},
): Promise<void> {
  const providers = seed.providers ?? defaultProviders
  const assistants = seed.assistants ?? defaultAssistants
  const workflows = seed.workflows ?? []
  const servers = seed.servers ?? []

  await page.route(/\/api\/user-llm-providers(\?|$)/, route =>
    route.fulfill({ status: 200, json: { providers } }),
  )
  await page.route(/\/api\/assistants(\?|$)/, route =>
    route.fulfill({
      status: 200,
      json: { assistants, total: assistants.length, page: 1, per_page: 100 },
    }),
  )
  await page.route(/\/api\/workflows(\?|$)/, route =>
    route.fulfill({
      status: 200,
      json: { workflows, total: workflows.length },
    }),
  )
  await page.route(/\/api\/mcp\/servers(\?|$)/, route =>
    route.fulfill({ status: 200, json: { servers } }),
  )
}

/** Select a value in a kit Select (Model picker): open the trigger, click the
 *  derived option testid. */
export async function pickSelect(
  page: Page,
  triggerTestid: string,
  value: string,
): Promise<void> {
  await byTestId(page, triggerTestid).click()
  const opt = byTestId(page, `${triggerTestid}-opt-${value}`)
  await opt.waitFor({ state: 'visible', timeout: 10000 })
  await opt.click()
}

/** Select a value in a kit Combobox (Assistant / Workflow): open, filter by text,
 *  commit via keyboard (the robust path for a Base UI combobox floating option). */
export async function pickCombobox(
  page: Page,
  inputTestid: string,
  filterText: string,
  value: string,
): Promise<void> {
  const input = byTestId(page, inputTestid)
  await input.click()
  await input.fill(filterText)
  const opt = byTestId(page, `${inputTestid}-opt-${value}`)
  await opt.waitFor({ state: 'visible', timeout: 10000 })
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('Enter')
}

/** Toggle an entry in a kit MultiSelect (allowed-tools): open, click the option,
 *  then close the popover so it doesn't overlay the Create button. We dismiss via
 *  an outside-press on `dismissTestid` (the name field) rather than Escape —
 *  inside a drawer Escape can bubble and close the whole drawer. */
export async function pickMultiSelect(
  page: Page,
  rootTestid: string,
  value: string,
  dismissTestid = 'task-form-name',
): Promise<void> {
  await byTestId(page, rootTestid).click()
  const opt = byTestId(page, `${rootTestid}-opt-${value}`)
  await opt.waitFor({ state: 'visible', timeout: 10000 })
  await opt.click()
  // Outside-press closes the popover without touching the drawer's Escape handler.
  await byTestId(page, dismissTestid).click()
  await opt.waitFor({ state: 'hidden', timeout: 5000 }).catch(() => {})
}
