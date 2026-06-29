import { Page, expect, Locator } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * Helpers for managing LLM provider <-> User group relationships
 * (kit / data-testid based)
 */

// A user-group card scoped by the (dynamic) group name it contains.
function groupCard(page: Page, groupName: string): Locator {
  return page
    .locator('[data-testid^="user-group-card-"]')
    .filter({ hasText: groupName })
    .first()
}

// The LLM-providers widget for a given group (carries an edit button whose
// aria-label embeds the group name).
function groupProvidersWidget(page: Page, groupName: string): Locator {
  return page
    .locator(`[data-widget="llm-providers"]:has([aria-label="Edit LLM Providers for ${groupName}"])`)
    .first()
}

// =====================================================
// User Group Navigation
// =====================================================

export async function goToUserGroupsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/user-groups`)
  await page.waitForLoadState('load')
  await waitForUserGroupsPageLoad(page)
}

export async function waitForUserGroupsPageLoad(page: Page) {
  await byTestId(page, 'user-groups-create-button').waitFor({ state: 'visible', timeout: 30000 })
  await page.waitForLoadState('load')
  // Allow lazy widgets + API calls to settle.
  await page.waitForTimeout(2000)
}

export async function clickGroupItem(page: Page, groupName: string) {
  const card = groupCard(page, groupName)
  await card.waitFor({ state: 'visible', timeout: 10000 })
  await card.scrollIntoViewIfNeeded()
  // Wait for lazy-loaded widgets (LLM Providers widget is lazy-loaded).
  await page.waitForTimeout(4000)
}

// =====================================================
// Provider Assignment in User Groups (Widget + Drawer)
// =====================================================

export async function openProviderAssignmentDrawerFromGroup(page: Page, groupName: string) {
  await clickGroupItem(page, groupName)

  const editButton = page.locator(`[aria-label="Edit LLM Providers for ${groupName}"]`).first()
  await editButton.waitFor({ state: 'visible', timeout: 10000 })
  await editButton.click()

  // Drawer is open once the save button renders.
  await byTestId(page, 'llm-group-providers-save-btn').waitFor({ state: 'visible', timeout: 5000 })
}

export async function toggleProviderInDrawer(page: Page, providerName: string, enable: boolean) {
  const providerCard = page
    .locator('[data-testid^="llm-group-provider-card-"]')
    .filter({ hasText: providerName })
    .first()
  await providerCard.waitFor({ state: 'visible', timeout: 5000 })

  const switchElement = providerCard.locator('[data-testid^="llm-group-provider-switch-"]').first()
  const isChecked = (await switchElement.getAttribute('aria-checked')) === 'true'
  if (isChecked !== enable) {
    await switchElement.click()
    await page.waitForTimeout(300)
  }
}

export async function saveProviderAssignment(page: Page) {
  const saveBtn = byTestId(page, 'llm-group-providers-save-btn')
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/groups\/[^/]+\/providers/.test(r.url()) && r.request().method() === 'PUT',
      { timeout: 10000 }
    ),
    saveBtn.click(),
  ])
  expect(resp.ok()).toBeTruthy()
  // Drawer closes on success.
  await saveBtn.waitFor({ state: 'hidden', timeout: 5000 })
  await page.waitForTimeout(1000)
}

export async function cancelProviderAssignment(page: Page) {
  await byTestId(page, 'llm-group-providers-cancel-btn').click()
  await byTestId(page, 'llm-group-providers-save-btn').waitFor({ state: 'hidden', timeout: 5000 })
}

export async function assignProviderToGroup(page: Page, groupName: string, providerName: string) {
  await waitForUserGroupsPageLoad(page)
  await openProviderAssignmentDrawerFromGroup(page, groupName)
  await toggleProviderInDrawer(page, providerName, true)
  await saveProviderAssignment(page)
}

export async function removeProviderFromGroup(page: Page, groupName: string, providerName: string) {
  await waitForUserGroupsPageLoad(page)
  await openProviderAssignmentDrawerFromGroup(page, groupName)
  await toggleProviderInDrawer(page, providerName, false)
  await saveProviderAssignment(page)
}

export async function assertProviderInGroupWidget(page: Page, groupName: string, providerName: string) {
  await waitForUserGroupsPageLoad(page)
  await clickGroupItem(page, groupName)

  const widget = groupProvidersWidget(page, groupName)
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  const providerTag = widget
    .locator('[data-testid="provider-tags-container"] [data-testid^="llm-provider-group-widget-tag-"]')
    .filter({ hasText: providerName })
  await expect(providerTag).toBeVisible()
}

export async function assertProviderNotInGroupWidget(page: Page, groupName: string, providerName: string) {
  await waitForUserGroupsPageLoad(page)
  await clickGroupItem(page, groupName)

  const widget = groupProvidersWidget(page, groupName)
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  const providerTag = widget
    .locator('[data-testid="provider-tags-container"] [data-testid^="llm-provider-group-widget-tag-"]')
    .filter({ hasText: providerName })
  await expect(providerTag).not.toBeVisible()
}

export async function assertGroupWidgetShowsCount(page: Page, groupName: string, expectedCount: number) {
  await waitForUserGroupsPageLoad(page)
  await clickGroupItem(page, groupName)

  const widget = groupProvidersWidget(page, groupName)
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  const tags = widget.locator('[data-testid^="llm-provider-group-widget-tag-"]')
  await expect(tags).toHaveCount(expectedCount)
}

// =====================================================
// Group Assignment in Providers (Card + Drawer)
// =====================================================

export async function openGroupAssignmentDrawerFromProvider(page: Page) {
  // Should already be on provider detail page.
  await byTestId(page, 'llm-provider-groups-card').waitFor({ state: 'visible', timeout: 10000 })
  await byTestId(page, 'llm-provider-groups-manage-btn').click()
  await byTestId(page, 'llm-provider-groups-save-btn').waitFor({ state: 'visible', timeout: 5000 })
}

export async function toggleGroupInDrawer(page: Page, groupName: string, enable: boolean) {
  const groupContainer = page
    .locator('[data-testid^="llm-provider-group-card-"]')
    .filter({ hasText: groupName })
    .first()
  await groupContainer.waitFor({ state: 'visible', timeout: 5000 })

  const switchElement = groupContainer.locator('[data-testid^="llm-provider-group-switch-"]').first()
  const isChecked = (await switchElement.getAttribute('aria-checked')) === 'true'
  if (isChecked !== enable) {
    await switchElement.click()
    await page.waitForTimeout(300)
  }
}

export async function saveGroupAssignment(page: Page) {
  const saveBtn = byTestId(page, 'llm-provider-groups-save-btn')
  await saveBtn.click()
  // Drawer closes on a successful save.
  await saveBtn.waitFor({ state: 'hidden', timeout: 10000 })
  await page.waitForTimeout(1000)
}

export async function cancelGroupAssignment(page: Page) {
  await byTestId(page, 'llm-provider-groups-cancel-btn').click()
  await byTestId(page, 'llm-provider-groups-save-btn').waitFor({ state: 'hidden', timeout: 5000 })
}

export async function assignGroupToProvider(page: Page, _providerId: string, groupName: string) {
  await openGroupAssignmentDrawerFromProvider(page)
  await toggleGroupInDrawer(page, groupName, true)
  await saveGroupAssignment(page)
}

export async function removeGroupFromProvider(page: Page, _providerId: string, groupName: string) {
  await openGroupAssignmentDrawerFromProvider(page)
  await toggleGroupInDrawer(page, groupName, false)
  await saveGroupAssignment(page)
}

export async function assertGroupInProviderCard(page: Page, groupName: string) {
  // Reload to force the event-only provider-groups widget to re-fetch.
  await page.reload({ waitUntil: 'load' })
  const card = byTestId(page, 'llm-provider-groups-card')
  await card.waitFor({ state: 'visible', timeout: 5000 })

  const groupTag = card
    .locator('[data-testid^="llm-provider-assigned-group-tag-"]')
    .filter({ hasText: groupName })
    .first()
  await expect(groupTag).toBeVisible({ timeout: 10000 })
}

export async function assertGroupNotInProviderCard(page: Page, groupName: string) {
  await page.reload({ waitUntil: 'load' })
  const card = byTestId(page, 'llm-provider-groups-card')
  await card.waitFor({ state: 'visible', timeout: 5000 })

  const groupTag = card
    .locator('[data-testid^="llm-provider-assigned-group-tag-"]')
    .filter({ hasText: groupName })
    .first()
  await expect(groupTag).not.toBeVisible()
}

export async function assertProviderCardShowsCount(page: Page, expectedCount: number) {
  await page.reload({ waitUntil: 'load' })
  const card = byTestId(page, 'llm-provider-groups-card')
  await card.waitFor({ state: 'visible', timeout: 5000 })

  const tags = card.locator('[data-testid^="llm-provider-assigned-group-tag-"]')
  await expect(tags).toHaveCount(expectedCount)
}

// =====================================================
// User Group Creation (for test setup)
// =====================================================

export async function createUserGroup(
  page: Page,
  baseURL: string,
  groupName: string,
  description?: string
): Promise<void> {
  await goToUserGroupsPage(page, baseURL)
  await waitForUserGroupsPageLoad(page)

  await byTestId(page, 'user-groups-create-button').click()
  await byTestId(page, 'user-create-group-form').waitFor({ state: 'visible', timeout: 5000 })

  await byTestId(page, 'user-create-group-name-input').fill(groupName)
  if (description) {
    await byTestId(page, 'user-create-group-description-textarea').fill(description)
  }

  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/groups/.test(r.url()) && r.request().method() === 'POST',
      { timeout: 10000 }
    ),
    byTestId(page, 'user-create-group-submit-button').click(),
  ])
  expect(resp.ok()).toBeTruthy()

  await byTestId(page, 'user-create-group-form').waitFor({ state: 'hidden', timeout: 5000 })
  await expect(groupCard(page, groupName)).toBeVisible()
}

export async function deleteUserGroup(page: Page, groupName: string): Promise<void> {
  await clickGroupItem(page, groupName)

  const card = groupCard(page, groupName)
  await card.locator('[data-testid^="user-group-delete-button-"]').first().click()

  // Kit Confirm — confirm button ends with `-confirm`.
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/groups/.test(r.url()) && r.request().method() === 'DELETE',
      { timeout: 10000 }
    ),
    page
      .locator('[data-testid^="user-group-delete-confirm-"][data-testid$="-confirm"]')
      .first()
      .click(),
  ])
  expect(resp.ok()).toBeTruthy()

  await expect(groupCard(page, groupName)).not.toBeVisible()
}
