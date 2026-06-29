import { Page, Locator, expect } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * Helpers for managing MCP server <-> User group relationships (kit/testid).
 *
 * Components are keyed by entity id (user-group-card-<id>, mcp-group-widget-*,
 * mcp-groups-*); the helpers receive names, so we bridge name → element by
 * filtering the id-keyed testid on the dynamic name text the test created.
 */

// =====================================================
// User Group Navigation
// =====================================================

export async function goToUserGroupsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/user-groups`)
  await page.waitForLoadState('load')
  await waitForUserGroupsPageLoad(page)
}

export async function waitForUserGroupsPageLoad(page: Page) {
  // The create button only renders once the groups page is interactive.
  await byTestId(page, 'user-groups-create-button').waitFor({ state: 'visible', timeout: 30000 })
}

/** Locate the group card carrying the given (dynamic) group name. */
function groupCardByName(page: Page, groupName: string): Locator {
  return page.getByTestId(/^user-group-card-/).filter({ hasText: groupName }).first()
}

export async function clickGroupItem(page: Page, groupName: string) {
  const card = groupCardByName(page, groupName)
  await card.waitFor({ state: 'visible', timeout: 10000 })
  await card.scrollIntoViewIfNeeded()
}

// =====================================================
// MCP Server Assignment in User Groups (Widget + Drawer)
// =====================================================

export async function openServerAssignmentDrawerFromGroup(page: Page, groupName: string) {
  const card = groupCardByName(page, groupName)
  await card.waitFor({ state: 'visible', timeout: 10000 })
  await card.scrollIntoViewIfNeeded()
  // The System-MCP-servers widget's edit button lives inside the group card.
  await card.getByTestId(/^mcp-group-widget-edit-btn-/).click()
  // Drawer open = its save button is present.
  await byTestId(page, 'mcp-group-assign-save-btn').waitFor({ state: 'visible', timeout: 5000 })
}

export async function toggleServerInDrawer(page: Page, serverName: string, enable: boolean) {
  // The drawer's per-server card carries the server's display name.
  const serverCard = page
    .getByTestId(/^mcp-group-assign-card-/)
    .filter({ hasText: serverName })
    .first()
  await serverCard.waitFor({ state: 'visible', timeout: 5000 })
  const switchElement = serverCard.getByTestId(/^mcp-group-assign-switch-/)
  const isChecked = (await switchElement.getAttribute('aria-checked')) === 'true'
  if (isChecked !== enable) {
    await switchElement.click()
  }
}

export async function saveServerAssignment(page: Page) {
  await byTestId(page, 'mcp-group-assign-save-btn').click()
  // Drawer closes on success → the save button leaves the DOM.
  await byTestId(page, 'mcp-group-assign-save-btn').waitFor({ state: 'detached', timeout: 10000 })
}

export async function cancelServerAssignment(page: Page) {
  await byTestId(page, 'mcp-group-assign-cancel-btn').click()
  await byTestId(page, 'mcp-group-assign-cancel-btn').waitFor({ state: 'detached', timeout: 5000 })
}

export async function assignServerToGroup(page: Page, groupName: string, serverName: string) {
  await waitForUserGroupsPageLoad(page)
  await openServerAssignmentDrawerFromGroup(page, groupName)
  await toggleServerInDrawer(page, serverName, true)
  await saveServerAssignment(page)
}

export async function removeServerFromGroup(page: Page, groupName: string, serverName: string) {
  await waitForUserGroupsPageLoad(page)
  await openServerAssignmentDrawerFromGroup(page, groupName)
  await toggleServerInDrawer(page, serverName, false)
  await saveServerAssignment(page)
}

export async function assertServerInGroupWidget(page: Page, groupName: string, serverName: string) {
  await waitForUserGroupsPageLoad(page)
  await clickGroupItem(page, groupName)
  const card = groupCardByName(page, groupName)
  const serverTag = card
    .getByTestId(/^mcp-group-widget-server-tag-/)
    .filter({ hasText: serverName })
  await expect(serverTag).toBeVisible()
}

export async function assertServerNotInGroupWidget(page: Page, groupName: string, serverName: string) {
  await waitForUserGroupsPageLoad(page)
  await clickGroupItem(page, groupName)
  const card = groupCardByName(page, groupName)
  const serverTag = card
    .getByTestId(/^mcp-group-widget-server-tag-/)
    .filter({ hasText: serverName })
  await expect(serverTag).toHaveCount(0)
}

export async function assertGroupWidgetShowsCount(page: Page, groupName: string, expectedCount: number) {
  await waitForUserGroupsPageLoad(page)
  await clickGroupItem(page, groupName)
  const card = groupCardByName(page, groupName)
  await expect(card.getByTestId(/^mcp-group-widget-server-tag-/)).toHaveCount(expectedCount)
}

// =====================================================
// Group Assignment in MCP Servers (Card + Drawer)
// =====================================================

/** The user-groups assignment accordion for a server (by name, or the first). */
function groupsWidgetForServer(page: Page, serverDisplayName?: string): Locator {
  if (serverDisplayName) {
    return page
      .getByTestId(/^mcp-system-server-card-/)
      .filter({ hasText: serverDisplayName })
      .first()
      .getByTestId(/^mcp-groups-accordion-/)
  }
  return page.getByTestId(/^mcp-groups-accordion-/).first()
}

/** Expand the accordion so the assigned-groups list (or empty state) shows. */
async function expandGroupsCollapseFor(page: Page, serverDisplayName?: string) {
  const widget = groupsWidgetForServer(page, serverDisplayName)
  await widget.waitFor({ state: 'visible', timeout: 10000 })
  const trigger = widget.getByRole('button').first()
  const expanded = (await trigger.getAttribute('aria-expanded')) === 'true'
  if (!expanded) {
    await trigger.click()
  }
}

export async function openGroupAssignmentDrawerFromServer(page: Page, serverDisplayName?: string) {
  const widget = groupsWidgetForServer(page, serverDisplayName)
  await widget.scrollIntoViewIfNeeded()
  await widget.waitFor({ state: 'visible', timeout: 10000 })
  await widget.getByTestId(/^mcp-groups-assign-btn-/).click()
  await byTestId(page, 'mcp-groups-drawer-save-btn').waitFor({ state: 'visible', timeout: 5000 })
}

export async function toggleGroupInDrawer(page: Page, groupName: string, enable: boolean) {
  const groupCard = page
    .getByTestId(/^mcp-groups-drawer-card-/)
    .filter({ hasText: groupName })
    .first()
  await groupCard.waitFor({ state: 'visible', timeout: 5000 })
  const switchElement = groupCard.getByTestId(/^mcp-groups-drawer-switch-/)
  const isChecked = (await switchElement.getAttribute('aria-checked')) === 'true'
  if (isChecked !== enable) {
    await switchElement.click()
  }
}

export async function saveGroupAssignment(page: Page) {
  await byTestId(page, 'mcp-groups-drawer-save-btn').click()
  await byTestId(page, 'mcp-groups-drawer-save-btn').waitFor({ state: 'detached', timeout: 10000 })
}

export async function cancelGroupAssignment(page: Page) {
  await byTestId(page, 'mcp-groups-drawer-cancel-btn').click()
  await byTestId(page, 'mcp-groups-drawer-cancel-btn').waitFor({ state: 'detached', timeout: 5000 })
}

export async function assignGroupToServer(
  page: Page,
  _serverId: string,
  groupName: string,
  serverDisplayName?: string,
) {
  await openGroupAssignmentDrawerFromServer(page, serverDisplayName)
  await toggleGroupInDrawer(page, groupName, true)
  await saveGroupAssignment(page)
}

export async function removeGroupFromServer(
  page: Page,
  _serverId: string,
  groupName: string,
  serverDisplayName?: string,
) {
  await openGroupAssignmentDrawerFromServer(page, serverDisplayName)
  await toggleGroupInDrawer(page, groupName, false)
  await saveGroupAssignment(page)
}

export async function assertGroupInServerCard(page: Page, groupName: string, serverDisplayName?: string) {
  await expandGroupsCollapseFor(page, serverDisplayName)
  const widget = groupsWidgetForServer(page, serverDisplayName)
  const groupTag = widget.getByTestId(/^mcp-group-tag-/).filter({ hasText: groupName })
  await expect(groupTag).toBeVisible()
}

export async function assertGroupNotInServerCard(page: Page, groupName: string, serverDisplayName?: string) {
  await expandGroupsCollapseFor(page, serverDisplayName)
  const widget = groupsWidgetForServer(page, serverDisplayName)
  const groupTag = widget.getByTestId(/^mcp-group-tag-/).filter({ hasText: groupName })
  await expect(groupTag).toHaveCount(0)
}

export async function assertServerCardShowsCount(page: Page, expectedCount: number, serverDisplayName?: string) {
  await expandGroupsCollapseFor(page, serverDisplayName)
  const widget = groupsWidgetForServer(page, serverDisplayName)
  await expect(widget.getByTestId(/^mcp-group-tag-/)).toHaveCount(expectedCount)
}

// =====================================================
// User Group Creation (for test setup)
// =====================================================

export async function createUserGroup(
  page: Page,
  baseURL: string,
  groupName: string,
  description?: string,
): Promise<void> {
  await goToUserGroupsPage(page, baseURL)
  await waitForUserGroupsPageLoad(page)

  await byTestId(page, 'user-groups-create-button').click()
  await byTestId(page, 'user-create-group-form').waitFor({ state: 'visible', timeout: 5000 })

  await byTestId(page, 'user-create-group-name-input').fill(groupName)
  if (description) {
    await byTestId(page, 'user-create-group-description-textarea').fill(description)
  }

  await byTestId(page, 'user-create-group-submit-button').click()
  // Drawer closes on success.
  await byTestId(page, 'user-create-group-form').waitFor({ state: 'detached', timeout: 10000 })

  // Verify the new group appears in the list.
  await expect(groupCardByName(page, groupName)).toBeVisible()
}

export async function deleteUserGroup(page: Page, groupName: string): Promise<void> {
  const card = groupCardByName(page, groupName)
  await card.waitFor({ state: 'visible', timeout: 10000 })
  await card.scrollIntoViewIfNeeded()

  await card.getByTestId(/^user-group-delete-button-/).click()
  // Confirm dialog OK button is `${confirmTestid}-confirm`.
  await page.getByTestId(/^user-group-delete-confirm-.+-confirm$/).click()

  // The group card leaves the DOM.
  await expect(card).toHaveCount(0)
}
