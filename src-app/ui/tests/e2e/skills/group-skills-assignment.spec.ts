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
  seedSystemSkill,
  openSkillAssignmentDrawerFromGroup,
  toggleSkillInDrawer,
  skillSwitchChecked,
  saveSkillAssignment,
  cancelSkillAssignment,
  assertSkillInGroupWidget,
  assertSkillNotInGroupWidget,
  assertGroupWidgetShowsSkillCount,
} from './helpers/group-skill-helpers'

/**
 * System Skills ↔ User-group assignment widget on /settings/user-groups.
 * Mirrors mcp/group-mcp-servers-assignment.spec.ts. E-SK-1 … E-SK-10.
 */
test.describe('System Skills assignment in User Groups', () => {
  // E-SK-1
  test('widget renders per group with a count', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-widget-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, `sk-w-${Date.now()}`)
    await createUserGroup(page, baseURL, group, 'widget render')

    await clickGroupItem(page, group)
    const card = page.getByTestId(/^user-group-card-/).filter({ hasText: group }).first()
    await expect(card.getByTestId(/^skill-group-widget-card-/)).toBeVisible({ timeout: 15000 })
    await expect(card.getByText('System Skills')).toBeVisible()
    await deleteUserGroup(page, group)
  })

  // E-SK-2
  test('user groups page with the widget passes a11y', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-a11y-${Date.now()}`
    const skill = `sk-a11y-skill-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, skill)
    await createUserGroup(page, baseURL, group, 'a11y')

    await goToUserGroupsPage(page, baseURL)
    await openSkillAssignmentDrawerFromGroup(page, group)
    await toggleSkillInDrawer(page, skill, true)
    await saveSkillAssignment(page)

    await goToUserGroupsPage(page, baseURL)
    await clickGroupItem(page, group)
    await assertNoAccessibilityViolations(page, { disabledRules: ['color-contrast'] })
    await deleteUserGroup(page, group)
  })

  // E-SK-3
  test('edit button carries the aria-label', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-aria-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, `sk-aria-skill-${Date.now()}`)
    await createUserGroup(page, baseURL, group, 'aria')

    await clickGroupItem(page, group)
    const card = page.getByTestId(/^user-group-card-/).filter({ hasText: group }).first()
    const editBtn = card.getByTestId(/^skill-group-widget-edit-btn-/)
    await expect(editBtn).toBeVisible({ timeout: 15000 })
    await expect(editBtn).toHaveAttribute('aria-label', `Edit System Skills for ${group}`)
    await deleteUserGroup(page, group)
  })

  // E-SK-4
  test('clicking edit opens a drawer listing all system skills', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-open-${Date.now()}`
    const skill = `sk-open-skill-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, skill)
    await createUserGroup(page, baseURL, group, 'open drawer')

    await openSkillAssignmentDrawerFromGroup(page, group)
    const skillCard = page.getByTestId(/^skill-group-assign-card-/).filter({ hasText: skill })
    await expect(skillCard).toBeVisible()
    await expect(skillCard.getByTestId(/^skill-group-assign-switch-/)).toBeVisible()
    await cancelSkillAssignment(page)
    await deleteUserGroup(page, group)
  })

  // E-SK-5
  test('an already-assigned skill shows its switch ON', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-pre-${Date.now()}`
    const skill = `sk-pre-skill-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, skill)
    await createUserGroup(page, baseURL, group, 'precheck')

    // Assign first.
    await openSkillAssignmentDrawerFromGroup(page, group)
    await toggleSkillInDrawer(page, skill, true)
    await saveSkillAssignment(page)

    // Reopen → switch is ON.
    await openSkillAssignmentDrawerFromGroup(page, group)
    expect(await skillSwitchChecked(page, skill)).toBe(true)
    await cancelSkillAssignment(page)
    await deleteUserGroup(page, group)
  })

  // E-SK-6
  test('assign + save reflects in the widget with an incremented count', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-assign-${Date.now()}`
    const skill = `sk-assign-skill-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, skill)
    await createUserGroup(page, baseURL, group, 'assign')

    await openSkillAssignmentDrawerFromGroup(page, group)
    await toggleSkillInDrawer(page, skill, true)
    await saveSkillAssignment(page)

    await clickGroupItem(page, group)
    await assertSkillInGroupWidget(page, group, skill)
    await assertGroupWidgetShowsSkillCount(page, group, 1)
    await deleteUserGroup(page, group)
  })

  // E-SK-7
  test('remove + save decrements the widget', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-remove-${Date.now()}`
    const skill = `sk-remove-skill-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, skill)
    await createUserGroup(page, baseURL, group, 'remove')

    await openSkillAssignmentDrawerFromGroup(page, group)
    await toggleSkillInDrawer(page, skill, true)
    await saveSkillAssignment(page)
    await clickGroupItem(page, group)
    await assertGroupWidgetShowsSkillCount(page, group, 1)

    await openSkillAssignmentDrawerFromGroup(page, group)
    await toggleSkillInDrawer(page, skill, false)
    await saveSkillAssignment(page)
    await clickGroupItem(page, group)
    await assertSkillNotInGroupWidget(page, group, skill)
    await assertGroupWidgetShowsSkillCount(page, group, 0)
    await deleteUserGroup(page, group)
  })

  // E-SK-8
  test('multi-assign shows both tags', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-multi-${Date.now()}`
    const skillA = `sk-multi-a-${Date.now()}`
    const skillB = `sk-multi-b-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, skillA)
    await seedSystemSkill(request, apiURL, token, skillB)
    await createUserGroup(page, baseURL, group, 'multi')

    await openSkillAssignmentDrawerFromGroup(page, group)
    await toggleSkillInDrawer(page, skillA, true)
    await toggleSkillInDrawer(page, skillB, true)
    await saveSkillAssignment(page)

    await clickGroupItem(page, group)
    await assertSkillInGroupWidget(page, group, skillA)
    await assertSkillInGroupWidget(page, group, skillB)
    await assertGroupWidgetShowsSkillCount(page, group, 2)
    await deleteUserGroup(page, group)
  })

  // E-SK-9
  test('cancel discards the change', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-cancel-${Date.now()}`
    const skill = `sk-cancel-skill-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedSystemSkill(request, apiURL, token, skill)
    await createUserGroup(page, baseURL, group, 'cancel')

    await openSkillAssignmentDrawerFromGroup(page, group)
    await toggleSkillInDrawer(page, skill, true)
    await cancelSkillAssignment(page)

    // Nothing persisted: widget empty + reopen shows switch OFF.
    await clickGroupItem(page, group)
    await assertSkillNotInGroupWidget(page, group, skill)
    await openSkillAssignmentDrawerFromGroup(page, group)
    expect(await skillSwitchChecked(page, skill)).toBe(false)
    await cancelSkillAssignment(page)
    await deleteUserGroup(page, group)
  })

  // E-SK-10
  test('a group with no skills shows the empty line', async ({ page, request, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const group = `sk-empty-${Date.now()}`
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    // Seed a system skill so the drawer has content, but assign none.
    await seedSystemSkill(request, apiURL, token, `sk-empty-skill-${Date.now()}`)
    await createUserGroup(page, baseURL, group, 'empty')

    await clickGroupItem(page, group)
    const card = page.getByTestId(/^user-group-card-/).filter({ hasText: group }).first()
    await expect(card.getByTestId(/^skill-group-widget-card-/)).toBeVisible({ timeout: 15000 })
    await expect(card.getByText('No System Skills assigned')).toBeVisible()
    await expect(card.getByTestId(/^skill-group-widget-tag-/)).toHaveCount(0)
    await deleteUserGroup(page, group)
  })
})
