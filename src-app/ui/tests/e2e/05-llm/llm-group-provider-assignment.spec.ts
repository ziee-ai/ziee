import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToUserGroupsPage,
  createUserGroup,
  deleteUserGroup,
  openGroupAssignmentDrawerFromProvider,
  toggleGroupInDrawer,
  saveGroupAssignment,
  cancelGroupAssignment,
  assignGroupToProvider,
  removeGroupFromProvider,
  assertGroupInProviderCard,
  assertGroupNotInProviderCard,
  assertProviderCardShowsCount,
} from './helpers/group-provider-helpers'
import {
  createLocalProvider,
  createRemoteProvider,
  deleteProvider,
} from './helpers/provider-helpers'
import { goToProvidersPage, clickProviderCard } from './helpers/navigation-helpers'

test.describe('User Group Assignment in LLM Providers', () => {
  test('should pass accessibility checks on provider detail page with card', async ({
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

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)

    // Check accessibility
    await assertNoAccessibilityViolations(page)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should display User Groups card in provider detail page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const providerName = `test-provider-card-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, providerName, 'Card display test')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)

    // Verify the card exists
    const card = page.locator('.ant-card:has(.ant-card-head-title:has-text("User Groups"))')
    await expect(card).toBeVisible()

    // Verify edit button exists
    const editButton = card.locator('button[aria-label="Manage user groups"]')
    await expect(editButton).toBeVisible()

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
  })

  test('should show empty state when no groups assigned', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-provider-empty-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, providerName, 'Empty state test')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)

    // Verify empty state
    await assertProviderCardShowsCount(page, 0)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
  })

  test('should open group assignment drawer from provider card', async ({
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

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)

    // Get provider ID from URL

    // Open drawer
    await openGroupAssignmentDrawerFromProvider(page)

    // Verify drawer is open with correct title
    await expect(
      page.locator(`.ant-drawer-title:has-text("Assign User Groups - ${providerName}")`)
    ).toBeVisible()

    // Verify group appears in the drawer
    await expect(page.locator(`.ant-drawer:visible:has-text("${groupName}")`)).toBeVisible()

    // Verify switch exists
    const groupCard = page.locator(
      `.ant-drawer:visible .ant-drawer-body .p-3:has-text("${groupName}")`
    )
    const switchElement = groupCard.locator('.ant-switch')
    await expect(switchElement).toBeVisible()

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should assign group to provider', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-assign-${Date.now()}`
    const providerName = `test-provider-assign-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Assignment test group')
    await createLocalProvider(page, baseURL, providerName, 'Assignment test provider')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)

    // Get provider ID
    const url = page.url()
    const providerId = url.split('/').pop()

    // Assign group
    await assignGroupToProvider(page, providerId!, groupName)

    // Verify group appears in card
    await assertGroupInProviderCard(page, groupName)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should remove group from provider', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-remove-${Date.now()}`
    const providerName = `test-provider-remove-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Removal test group')
    await createLocalProvider(page, baseURL, providerName, 'Removal test provider')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)

    // Get provider ID
    const url = page.url()
    const providerId = url.split('/').pop()

    // Assign then remove
    await assignGroupToProvider(page, providerId!, groupName)
    await removeGroupFromProvider(page, providerId!, groupName)

    // Verify group is gone from card
    await assertGroupNotInProviderCard(page, groupName)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should assign multiple groups to provider', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const group1 = `test-group-1-${Date.now()}`
    const group2 = `test-group-2-${Date.now()}`
    const providerName = `test-provider-multi-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, group1, 'Group 1')
    await createUserGroup(page, baseURL, group2, 'Group 2')
    await createLocalProvider(page, baseURL, providerName, 'Multiple groups test')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)


    // Assign both groups at once
    await openGroupAssignmentDrawerFromProvider(page)
    await toggleGroupInDrawer(page, group1, true)
    await toggleGroupInDrawer(page, group2, true)
    await saveGroupAssignment(page)

    // Verify both appear in card
    await assertGroupInProviderCard(page, group1)
    await assertGroupInProviderCard(page, group2)
    await assertProviderCardShowsCount(page, 2)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, group1)
    await deleteUserGroup(page, group2)
  })

  test('should work with remote providers', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-remote-${Date.now()}`
    const providerName = `test-provider-remote-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup - create remote provider
    await createUserGroup(page, baseURL, groupName, 'Remote provider test')
    await createRemoteProvider(
      page,
      baseURL,
      providerName,
      'https://api.openai.com/v1',
      'sk-test-key'
    )

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)

    // Get provider ID
    const url = page.url()
    const providerId = url.split('/').pop()

    // Verify User Groups card exists (should work for both local and remote)
    const card = page.locator('.ant-card:has(.ant-card-head-title:has-text("User Groups"))')
    await expect(card).toBeVisible()

    // Assign group
    await assignGroupToProvider(page, providerId!, groupName)
    await assertGroupInProviderCard(page, groupName)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should show system groups with tag', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-provider-system-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createLocalProvider(page, baseURL, providerName, 'System groups test')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)


    // Open drawer
    await openGroupAssignmentDrawerFromProvider(page)

    // Look for "All Users" (which is a system group)
    const allUsersCard = page.locator(
      `.ant-drawer:visible .ant-drawer-body .p-3:has-text("All Users")`
    )

    // If All Users exists, verify it has System tag
    const allUsersCount = await allUsersCard.count()
    if (allUsersCount > 0) {
      await expect(allUsersCard.locator('.ant-tag:has-text("System")')).toBeVisible()
    }

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
  })

  test('should show active/inactive status for groups', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-status-${Date.now()}`
    const providerName = `test-provider-status-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup - create active group
    await createUserGroup(page, baseURL, groupName, 'Status test group')
    await createLocalProvider(page, baseURL, providerName, 'Status test provider')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)


    // Open drawer
    await openGroupAssignmentDrawerFromProvider(page)

    // Find the group card
    const groupCard = page.locator(
      `.ant-drawer:visible .ant-drawer-body .p-3:has-text("${groupName}")`
    )
    await expect(groupCard).toBeVisible()

    // Verify it shows "Active" tag (groups are active by default)
    await expect(groupCard.locator('.ant-tag:has-text("Active")')).toBeVisible()

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should update card count when groups are added/removed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const group1 = `test-group-count-1-${Date.now()}`
    const group2 = `test-group-count-2-${Date.now()}`
    const providerName = `test-provider-count-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, group1, 'Group 1')
    await createUserGroup(page, baseURL, group2, 'Group 2')
    await createLocalProvider(page, baseURL, providerName, 'Count update test')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)

    // Get provider ID
    const url = page.url()
    const providerId = url.split('/').pop()

    // Start with 0
    await assertProviderCardShowsCount(page, 0)

    // Add one group -> count = 1
    await assignGroupToProvider(page, providerId!, group1)
    await assertProviderCardShowsCount(page, 1)

    // Add another group -> count = 2
    await assignGroupToProvider(page, providerId!, group2)
    await assertProviderCardShowsCount(page, 2)

    // Remove one group -> count = 1
    await removeGroupFromProvider(page, providerId!, group1)
    await assertProviderCardShowsCount(page, 1)

    // Remove last group -> count = 0
    await removeGroupFromProvider(page, providerId!, group2)
    await assertProviderCardShowsCount(page, 0)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, group1)
    await deleteUserGroup(page, group2)
  })

  test('should cancel assignment without saving changes', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-cancel-${Date.now()}`
    const providerName = `test-provider-cancel-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Cancel test group')
    await createLocalProvider(page, baseURL, providerName, 'Cancel test provider')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)


    // Open drawer and toggle group but cancel
    await openGroupAssignmentDrawerFromProvider(page)
    await toggleGroupInDrawer(page, groupName, true)
    await cancelGroupAssignment(page)

    // Verify group was NOT assigned
    await assertGroupNotInProviderCard(page, groupName)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should toggle group by clicking card', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-click-${Date.now()}`
    const providerName = `test-provider-click-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Click test group')
    await createLocalProvider(page, baseURL, providerName, 'Click test provider')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)


    // Open drawer
    await openGroupAssignmentDrawerFromProvider(page)

    // Get the group card and switch
    const groupCard = page.locator(
      `.ant-drawer:visible .ant-drawer-body .p-3:has-text("${groupName}")`
    )
    const switchElement = groupCard.locator('.ant-switch')

    // Verify initially unchecked
    await expect(switchElement).toHaveAttribute('aria-checked', 'false')

    // Click the card (not the switch)
    await groupCard.click()
    await page.waitForTimeout(300)

    // Verify switch is now checked
    await expect(switchElement).toHaveAttribute('aria-checked', 'true')

    // Click card again
    await groupCard.click()
    await page.waitForTimeout(300)

    // Verify switch is back to unchecked
    await expect(switchElement).toHaveAttribute('aria-checked', 'false')

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should show group description in drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-desc-${Date.now()}`
    const groupDescription = 'This is a test group description'
    const providerName = `test-provider-desc-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup - create group with description
    await createUserGroup(page, baseURL, groupName, groupDescription)
    await createLocalProvider(page, baseURL, providerName, 'Description test provider')

    // Navigate to provider detail page
    await goToProvidersPage(page, baseURL)
    await clickProviderCard(page, providerName)


    // Open drawer
    await openGroupAssignmentDrawerFromProvider(page)

    // Find the group card
    const groupCard = page.locator(
      `.ant-drawer:visible .ant-drawer-body .p-3:has-text("${groupName}")`
    )
    await expect(groupCard).toBeVisible()

    // Verify description is shown
    await expect(groupCard.locator(`text=${groupDescription}`)).toBeVisible()

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })
})
