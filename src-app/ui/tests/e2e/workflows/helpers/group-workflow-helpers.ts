import { type APIRequestContext, type Page, expect } from '@playwright/test'
import { byTestId } from '../../testid'
import { buildWorkflowBundle } from './workflow-helpers'

/**
 * Helpers for the System-Workflows ↔ User-group assignment widget/drawer,
 * mirrors `skills/helpers/group-skill-helpers.ts`. Widget testids:
 * `workflow-group-widget-card-<gid>` / `-edit-btn-<gid>` / `-tag-<wid>`;
 * drawer: `workflow-group-assign-card-<wid>` / `-switch-<wid>` / `-save-btn` /
 * `-cancel-btn`.
 */

const SIMPLE_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: noop
    kind: llm
    prompt: "say something about {{ inputs.topic }}"
outputs:
  - name: out
    from: "{{ noop.output }}"
    expose: full
`

/**
 * Install a SYSTEM-scope workflow via `POST /api/workflows/system/import`.
 * Returns the workflow's display name (the slug), which the widget tags +
 * drawer cards render.
 */
export async function seedSystemWorkflow(
  request: APIRequestContext,
  apiURL: string,
  token: string,
  slug: string,
): Promise<string> {
  const resp = await request.post(
    `${apiURL}/api/workflows/system/import?name=${encodeURIComponent(slug)}`,
    {
      headers: { Authorization: `Bearer ${token}` },
      multipart: {
        bundle: {
          name: 'bundle.tar.gz',
          mimeType: 'application/gzip',
          buffer: buildWorkflowBundle(SIMPLE_YAML),
        },
      },
    },
  )
  expect(resp.status(), `system workflow import should 201: ${await resp.text()}`).toBe(201)
  return slug
}

function groupCardByName(page: Page, groupName: string) {
  return page.getByTestId(/^user-group-card-/).filter({ hasText: groupName }).first()
}

export async function openWorkflowAssignmentDrawerFromGroup(page: Page, groupName: string) {
  const card = groupCardByName(page, groupName)
  await card.waitFor({ state: 'visible', timeout: 10000 })
  await card.scrollIntoViewIfNeeded()
  await card.getByTestId(/^workflow-group-widget-edit-btn-/).click()
  await byTestId(page, 'workflow-group-assign-save-btn').waitFor({ state: 'visible', timeout: 5000 })
}

export async function toggleWorkflowInDrawer(page: Page, workflowName: string, enable: boolean) {
  const wfCard = page
    .getByTestId(/^workflow-group-assign-card-/)
    .filter({ hasText: workflowName })
    .first()
  await wfCard.waitFor({ state: 'visible', timeout: 5000 })
  const sw = wfCard.getByTestId(/^workflow-group-assign-switch-/)
  const isChecked = (await sw.getAttribute('aria-checked')) === 'true'
  if (isChecked !== enable) {
    await sw.click()
  }
}

export async function workflowSwitchChecked(page: Page, workflowName: string): Promise<boolean> {
  const wfCard = page
    .getByTestId(/^workflow-group-assign-card-/)
    .filter({ hasText: workflowName })
    .first()
  await wfCard.waitFor({ state: 'visible', timeout: 5000 })
  const sw = wfCard.getByTestId(/^workflow-group-assign-switch-/)
  return (await sw.getAttribute('aria-checked')) === 'true'
}

export async function saveWorkflowAssignment(page: Page) {
  await byTestId(page, 'workflow-group-assign-save-btn').click()
  await byTestId(page, 'workflow-group-assign-save-btn').waitFor({ state: 'detached', timeout: 10000 })
}

export async function cancelWorkflowAssignment(page: Page) {
  await byTestId(page, 'workflow-group-assign-cancel-btn').click()
  await byTestId(page, 'workflow-group-assign-cancel-btn').waitFor({ state: 'detached', timeout: 5000 })
}

export async function assertWorkflowInGroupWidget(page: Page, groupName: string, workflowName: string) {
  const card = groupCardByName(page, groupName)
  const tag = card.getByTestId(/^workflow-group-widget-tag-/).filter({ hasText: workflowName })
  await expect(tag).toBeVisible()
}

export async function assertWorkflowNotInGroupWidget(page: Page, groupName: string, workflowName: string) {
  const card = groupCardByName(page, groupName)
  const tag = card.getByTestId(/^workflow-group-widget-tag-/).filter({ hasText: workflowName })
  await expect(tag).toHaveCount(0)
}

export async function assertGroupWidgetShowsWorkflowCount(
  page: Page,
  groupName: string,
  expectedCount: number,
) {
  const card = groupCardByName(page, groupName)
  await expect(card.getByTestId(/^workflow-group-widget-tag-/)).toHaveCount(expectedCount)
}
