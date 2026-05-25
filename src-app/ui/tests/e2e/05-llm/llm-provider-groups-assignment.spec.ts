import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToUserGroupsPage,
  createUserGroup,
  deleteUserGroup,
  clickGroupItem,
  openProviderAssignmentDrawerFromGroup,
  toggleProviderInDrawer,
  saveProviderAssignment,
  cancelProviderAssignment,
  assignProviderToGroup,
  removeProviderFromGroup,
  assertProviderInGroupWidget,
  assertProviderNotInGroupWidget,
  assertGroupWidgetShowsCount,
} from './helpers/group-provider-helpers'
import { createLocalProvider, deleteProvider } from './helpers/provider-helpers'
import { goToProvidersPage, waitForProvidersPageLoad } from './helpers/navigation-helpers'

test.describe('LLM Provider Assignment in User Groups', () => {
  test('should pass accessibility checks on user groups page with widget', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-a11y-${Date.now()}`
    const providerName = `test-provider-a11y-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup: Create group and provider
    await createUserGroup(page, baseURL, groupName, 'Accessibility test group')
    await createLocalProvider(page, baseURL, providerName, 'Accessibility test provider')

    // Assign provider to group
    await goToUserGroupsPage(page, baseURL)
    await assignProviderToGroup(page, groupName, providerName)

    // Check accessibility with widget visible
    await goToUserGroupsPage(page, baseURL)
    await clickGroupItem(page, groupName)
    // Disable color-contrast rule for AntD's orange tag (known limitation)
    await assertNoAccessibilityViolations(page, {
      disabledRules: ['color-contrast'],
    })

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, providerName)
  })

  test('should display LLM Provider widget in user group', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-widget-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createUserGroup(page, baseURL, groupName, 'Widget display test')

    // Wait for the group to be visible and scroll into view
    await clickGroupItem(page, groupName)

    // Wait for the specific widget to load for this group (longer timeout for lazy loading)
    // Use .first() to handle potential duplicate widgets
    const widget = page.locator(`[data-widget="llm-providers"]:has(button[aria-label="Edit LLM Providers for ${groupName}"])`).first()
    await widget.waitFor({ state: 'visible', timeout: 15000 })

    // Verify edit button exists with the specific aria-label
    const editButton = page.locator(`button[aria-label="Edit LLM Providers for ${groupName}"]`).first()
    await expect(editButton).toBeVisible()

    // Cleanup
    await deleteUserGroup(page, groupName)
  })

  test('should open provider assignment drawer from group widget', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-drawer-${Date.now()}`
    const providerName = `test-provider-drawer-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Drawer test group')
    await createLocalProvider(page, baseURL, providerName, 'Drawer test provider')

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openProviderAssignmentDrawerFromGroup(page, groupName)

    // Verify drawer is open with correct title
    await expect(
      page.locator(`.ant-drawer-title:has-text("Assign LLM Providers - ${groupName}")`)
    ).toBeVisible()

    // Verify provider appears in the drawer
    await expect(page.locator(`.ant-drawer.ant-drawer-open:has-text("${providerName}")`)).toBeVisible()

    // Verify switch exists
    const providerCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has-text("${providerName}")`
    )
    const switchElement = providerCard.locator('.ant-switch')
    await expect(switchElement).toBeVisible()

    // Close drawer
    await cancelProviderAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, providerName)
  })

  test('should assign provider to group', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-assign-${Date.now()}`
    const providerName = `test-provider-assign-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Assignment test group')
    await createLocalProvider(page, baseURL, providerName, 'Assignment test provider')

    // Assign provider
    await goToUserGroupsPage(page, baseURL)
    await assignProviderToGroup(page, groupName, providerName)

    // Verify provider appears in widget
    await goToUserGroupsPage(page, baseURL)
    await assertProviderInGroupWidget(page, groupName, providerName)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, providerName)
  })

  test('should remove provider from group', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-remove-${Date.now()}`
    const providerName = `test-provider-remove-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Removal test group')
    await createLocalProvider(page, baseURL, providerName, 'Removal test provider')

    // Assign then remove
    await goToUserGroupsPage(page, baseURL)
    await assignProviderToGroup(page, groupName, providerName)
    await removeProviderFromGroup(page, groupName, providerName)

    // Verify provider is gone from widget
    await goToUserGroupsPage(page, baseURL)
    await assertProviderNotInGroupWidget(page, groupName, providerName)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, providerName)
  })

  test('should assign multiple providers to group', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-multi-${Date.now()}`
    const provider1 = `test-provider-1-${Date.now()}`
    const provider2 = `test-provider-2-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Multiple providers test')
    await createLocalProvider(page, baseURL, provider1, 'Provider 1')
    await createLocalProvider(page, baseURL, provider2, 'Provider 2')

    // Assign both providers at once
    await goToUserGroupsPage(page, baseURL)
    await openProviderAssignmentDrawerFromGroup(page, groupName)
    await toggleProviderInDrawer(page, provider1, true)
    await toggleProviderInDrawer(page, provider2, true)
    await saveProviderAssignment(page)

    // Verify both appear in widget
    await goToUserGroupsPage(page, baseURL)
    await assertProviderInGroupWidget(page, groupName, provider1)
    await assertProviderInGroupWidget(page, groupName, provider2)
    await assertGroupWidgetShowsCount(page, groupName, 2)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, provider1)
    await deleteProvider(page, provider2)
  })

  test('should show disabled providers in drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-disabled-${Date.now()}`
    const providerName = `test-provider-disabled-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup - create disabled provider
    await createUserGroup(page, baseURL, groupName, 'Disabled provider test')
    await createLocalProvider(page, baseURL, providerName, 'Disabled test provider')

    // Disable the provider
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)
    const providerMenuItem = page.locator(`[role="menu"] [role="menuitem"]:has-text("${providerName}")`)
    await providerMenuItem.click()
    const toggle = page.locator(`.ant-switch[aria-label*="${providerName}"]`)
    await toggle.click()
    await page.waitForTimeout(500)

    // Open drawer and verify disabled provider is shown and can be assigned
    await goToUserGroupsPage(page, baseURL)
    await openProviderAssignmentDrawerFromGroup(page, groupName)

    // Verify provider appears even though it's disabled
    const providerCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has-text("${providerName}")`
    ).first()
    await expect(providerCard).toBeVisible()

    // Verify it shows "Disabled" tag
    await expect(providerCard.locator('.ant-tag:has-text("Disabled")').first()).toBeVisible()

    // Verify we can still toggle it
    const switchElement = providerCard.locator('.ant-switch').first()
    await expect(switchElement).toBeEnabled()

    // Close drawer
    await cancelProviderAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, providerName)
  })

  test('should update widget count when providers are added/removed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-count-${Date.now()}`
    const provider1 = `test-provider-count-1-${Date.now()}`
    const provider2 = `test-provider-count-2-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Count update test')
    await createLocalProvider(page, baseURL, provider1, 'Provider 1')
    await createLocalProvider(page, baseURL, provider2, 'Provider 2')

    // Start with 0
    await goToUserGroupsPage(page, baseURL)
    await assertGroupWidgetShowsCount(page, groupName, 0)

    // Add one provider -> count = 1
    await assignProviderToGroup(page, groupName, provider1)
    await goToUserGroupsPage(page, baseURL)
    await assertGroupWidgetShowsCount(page, groupName, 1)

    // Add another provider -> count = 2
    await assignProviderToGroup(page, groupName, provider2)
    await goToUserGroupsPage(page, baseURL)
    await assertGroupWidgetShowsCount(page, groupName, 2)

    // Remove one provider -> count = 1
    await removeProviderFromGroup(page, groupName, provider1)
    await goToUserGroupsPage(page, baseURL)
    await assertGroupWidgetShowsCount(page, groupName, 1)

    // Remove last provider -> count = 0
    await removeProviderFromGroup(page, groupName, provider2)
    await goToUserGroupsPage(page, baseURL)
    await assertGroupWidgetShowsCount(page, groupName, 0)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, provider1)
    await deleteProvider(page, provider2)
  })

  test('should cancel assignment without saving changes', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-cancel-${Date.now()}`
    const providerName = `test-provider-cancel-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Cancel test group')
    await createLocalProvider(page, baseURL, providerName, 'Cancel test provider')

    // Open drawer and toggle provider but cancel
    await goToUserGroupsPage(page, baseURL)
    await openProviderAssignmentDrawerFromGroup(page, groupName)
    await toggleProviderInDrawer(page, providerName, true)
    await cancelProviderAssignment(page)

    // Verify provider was NOT assigned
    await goToUserGroupsPage(page, baseURL)
    await assertProviderNotInGroupWidget(page, groupName, providerName)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, providerName)
  })

  test('should show built-in tag for built-in providers', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-builtin-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup - create group
    await createUserGroup(page, baseURL, groupName, 'Built-in provider test')

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openProviderAssignmentDrawerFromGroup(page, groupName)

    // Look for Ollama (which is built-in)
    const ollamaCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has-text("Ollama")`
    )

    // If Ollama exists, verify it has Built-in tag
    const ollamaCount = await ollamaCard.count()
    if (ollamaCount > 0) {
      await expect(ollamaCard.locator('.ant-tag:has-text("Built-in")')).toBeVisible()
    }

    // Close drawer
    await cancelProviderAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
  })

  test('should toggle provider switch', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-switch-${Date.now()}`
    const providerName = `test-provider-switch-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Switch test group')
    await createLocalProvider(page, baseURL, providerName, 'Switch test provider')

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openProviderAssignmentDrawerFromGroup(page, groupName)

    // Get the provider card and switch
    const providerCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has-text("${providerName}")`
    )
    const switchElement = providerCard.locator('.ant-switch')

    // Verify initially unchecked
    await expect(switchElement).toHaveAttribute('aria-checked', 'false')

    // Click the switch directly to toggle
    await switchElement.click()
    // Wait for React state update
    await page.waitForTimeout(300)

    // Verify switch is now checked
    await expect(switchElement).toHaveAttribute('aria-checked', 'true')

    // Click switch again
    await switchElement.click()
    await page.waitForTimeout(300)

    // Verify switch is back to unchecked
    await expect(switchElement).toHaveAttribute('aria-checked', 'false')

    // Close drawer
    await cancelProviderAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToProvidersPage(page, baseURL)
    await deleteProvider(page, providerName)
  })
})
