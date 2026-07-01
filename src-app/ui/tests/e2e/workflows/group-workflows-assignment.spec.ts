import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import {
  goToUserGroupsPage,
  createUserGroup,
  deleteUserGroup,
  clickGroupItem,
} from '../mcp/helpers/group-server-helpers'
import {
  seedSystemWorkflow,
  openWorkflowAssignmentDrawerFromGroup,
  toggleWorkflowInDrawer,
  workflowSwitchChecked,
  saveWorkflowAssignment,
  cancelWorkflowAssignment,
  assertWorkflowInGroupWidget,
  assertWorkflowNotInGroupWidget,
  assertGroupWidgetShowsWorkflowCount,
} from './helpers/group-workflow-helpers'

/**
 * System Workflows ↔ User-group assignment widget on /settings/user-groups.
 * Mirrors skills/group-skills-assignment.spec.ts. E-WF-1 … E-WF-10.
 */
test.describe('System Workflows assignment in User Groups', () => {
  // E-WF-1
  test('widget renders per group with a count', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-widget-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, `wf-w-${Date.now()}`)
    await createUserGroup(page, baseURL, group, 'widget render')

    await clickGroupItem(page, group)
    const card = page.getByTestId(/^user-group-card-/).filter({ hasText: group }).first()
    await expect(card.getByTestId(/^workflow-group-widget-card-/)).toBeVisible({ timeout: 15000 })
    await expect(card.getByText('System Workflows')).toBeVisible()
    await deleteUserGroup(page, group)
  })

  // E-WF-2
  test('user groups page with the widget passes a11y', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-a11y-${Date.now()}`
    const wf = `wf-a11y-flow-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, wf)
    await createUserGroup(page, baseURL, group, 'a11y')

    await goToUserGroupsPage(page, baseURL)
    await openWorkflowAssignmentDrawerFromGroup(page, group)
    await toggleWorkflowInDrawer(page, wf, true)
    await saveWorkflowAssignment(page)

    await goToUserGroupsPage(page, baseURL)
    await clickGroupItem(page, group)
    await assertNoAccessibilityViolations(page, { disabledRules: ['color-contrast'] })
    await deleteUserGroup(page, group)
  })

  // E-WF-3
  test('edit button carries the aria-label', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-aria-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, `wf-aria-flow-${Date.now()}`)
    await createUserGroup(page, baseURL, group, 'aria')

    await clickGroupItem(page, group)
    const card = page.getByTestId(/^user-group-card-/).filter({ hasText: group }).first()
    const editBtn = card.getByTestId(/^workflow-group-widget-edit-btn-/)
    await expect(editBtn).toBeVisible({ timeout: 15000 })
    await expect(editBtn).toHaveAttribute('aria-label', `Edit System Workflows for ${group}`)
    await deleteUserGroup(page, group)
  })

  // E-WF-4
  test('clicking edit opens a drawer listing all system workflows', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-open-${Date.now()}`
    const wf = `wf-open-flow-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, wf)
    await createUserGroup(page, baseURL, group, 'open drawer')

    await openWorkflowAssignmentDrawerFromGroup(page, group)
    const wfCard = page.getByTestId(/^workflow-group-assign-card-/).filter({ hasText: wf })
    await expect(wfCard).toBeVisible()
    await expect(wfCard.getByTestId(/^workflow-group-assign-switch-/)).toBeVisible()
    await cancelWorkflowAssignment(page)
    await deleteUserGroup(page, group)
  })

  // E-WF-5
  test('an already-assigned workflow shows its switch ON', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-pre-${Date.now()}`
    const wf = `wf-pre-flow-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, wf)
    await createUserGroup(page, baseURL, group, 'precheck')

    await openWorkflowAssignmentDrawerFromGroup(page, group)
    await toggleWorkflowInDrawer(page, wf, true)
    await saveWorkflowAssignment(page)

    await openWorkflowAssignmentDrawerFromGroup(page, group)
    expect(await workflowSwitchChecked(page, wf)).toBe(true)
    await cancelWorkflowAssignment(page)
    await deleteUserGroup(page, group)
  })

  // E-WF-6
  test('assign + save reflects in the widget with an incremented count', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-assign-${Date.now()}`
    const wf = `wf-assign-flow-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, wf)
    await createUserGroup(page, baseURL, group, 'assign')

    await openWorkflowAssignmentDrawerFromGroup(page, group)
    await toggleWorkflowInDrawer(page, wf, true)
    await saveWorkflowAssignment(page)

    await clickGroupItem(page, group)
    await assertWorkflowInGroupWidget(page, group, wf)
    await assertGroupWidgetShowsWorkflowCount(page, group, 1)
    await deleteUserGroup(page, group)
  })

  // E-WF-7
  test('remove + save decrements the widget', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-remove-${Date.now()}`
    const wf = `wf-remove-flow-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, wf)
    await createUserGroup(page, baseURL, group, 'remove')

    await openWorkflowAssignmentDrawerFromGroup(page, group)
    await toggleWorkflowInDrawer(page, wf, true)
    await saveWorkflowAssignment(page)
    await clickGroupItem(page, group)
    await assertGroupWidgetShowsWorkflowCount(page, group, 1)

    await openWorkflowAssignmentDrawerFromGroup(page, group)
    await toggleWorkflowInDrawer(page, wf, false)
    await saveWorkflowAssignment(page)
    await clickGroupItem(page, group)
    await assertWorkflowNotInGroupWidget(page, group, wf)
    await assertGroupWidgetShowsWorkflowCount(page, group, 0)
    await deleteUserGroup(page, group)
  })

  // E-WF-8
  test('multi-assign shows both tags', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-multi-${Date.now()}`
    const wfA = `wf-multi-a-${Date.now()}`
    const wfB = `wf-multi-b-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, wfA)
    await seedSystemWorkflow(request, apiURL, token, wfB)
    await createUserGroup(page, baseURL, group, 'multi')

    await openWorkflowAssignmentDrawerFromGroup(page, group)
    await toggleWorkflowInDrawer(page, wfA, true)
    await toggleWorkflowInDrawer(page, wfB, true)
    await saveWorkflowAssignment(page)

    await clickGroupItem(page, group)
    await assertWorkflowInGroupWidget(page, group, wfA)
    await assertWorkflowInGroupWidget(page, group, wfB)
    await assertGroupWidgetShowsWorkflowCount(page, group, 2)
    await deleteUserGroup(page, group)
  })

  // E-WF-9
  test('cancel discards the change', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-cancel-${Date.now()}`
    const wf = `wf-cancel-flow-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, wf)
    await createUserGroup(page, baseURL, group, 'cancel')

    await openWorkflowAssignmentDrawerFromGroup(page, group)
    await toggleWorkflowInDrawer(page, wf, true)
    await cancelWorkflowAssignment(page)

    await clickGroupItem(page, group)
    await assertWorkflowNotInGroupWidget(page, group, wf)
    await openWorkflowAssignmentDrawerFromGroup(page, group)
    expect(await workflowSwitchChecked(page, wf)).toBe(false)
    await cancelWorkflowAssignment(page)
    await deleteUserGroup(page, group)
  })

  // E-WF-10
  test('a group with no workflows shows the empty line', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `wf-empty-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemWorkflow(request, apiURL, token, `wf-empty-flow-${Date.now()}`)
    await createUserGroup(page, baseURL, group, 'empty')

    await clickGroupItem(page, group)
    const card = page.getByTestId(/^user-group-card-/).filter({ hasText: group }).first()
    await expect(card.getByTestId(/^workflow-group-widget-card-/)).toBeVisible({ timeout: 15000 })
    await expect(card.getByText('No System Workflows assigned')).toBeVisible()
    await expect(card.getByTestId(/^workflow-group-widget-tag-/)).toHaveCount(0)
    await deleteUserGroup(page, group)
  })
})
