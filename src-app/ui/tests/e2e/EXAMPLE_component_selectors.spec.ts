/**
 * Example test showing how to use component selectors
 *
 * This test demonstrates the automatic data-component-name attributes
 * that are added to all React components in dev/test mode.
 *
 * To run this test:
 * npm run test:e2e -- EXAMPLE_component_selectors.spec.ts
 */

import { test, expect } from '@playwright/test'
import {
  getComponent,
  getAllComponents,
  waitForComponent,
  clickComponent,
  isComponentVisible,
  getComponentWithin,
} from '../helpers/component-selectors'

test.describe('Component Selectors Example', () => {
  test.beforeEach(async ({ page }) => {
    // Login or navigate to starting page
    await page.goto('/settings/users')
  })

  test('example: basic component selection', async ({ page }) => {
    // Wait for a component to appear
    await waitForComponent(page, 'UsersSettings')

    // Get a component by name
    const usersSettings = getComponent(page, 'UsersSettings')
    await expect(usersSettings).toBeVisible()

    // You can still use regular Playwright selectors on the component
    const heading = usersSettings.getByRole('heading', { name: 'Users' })
    await expect(heading).toBeVisible()
  })

  test('example: working with lists', async ({ page }) => {
    // Get all instances of a component (e.g., list items)
    const userItems = getAllComponents(page, 'UserListItem')

    // Check how many instances exist
    const count = await userItems.count()
    expect(count).toBeGreaterThan(0)

    // Interact with the first item
    await expect(userItems.first()).toBeVisible()

    // Interact with a specific item
    await expect(userItems.nth(2)).toBeVisible()
  })

  test('example: working with drawers/modals', async ({ page }) => {
    // Click a button to open a drawer
    await clickComponent(page, 'CreateUserButton')

    // Wait for drawer to appear
    const drawer = await waitForComponent(page, 'CreateUserDrawer')

    // Get components within the drawer
    const form = getComponentWithin(drawer, 'UserForm')
    await expect(form).toBeVisible()

    // Fill form using regular Playwright selectors
    await drawer.getByLabel('Username').fill('testuser')
    await drawer.getByLabel('Email').fill('test@example.com')

    // Submit the form
    await drawer.getByRole('button', { name: 'Create User' }).click()

    // Check if drawer closed
    await expect(drawer).not.toBeVisible()
  })

  test('example: conditional component checks', async ({ page }) => {
    // Check if a component exists before interacting
    const hasErrorMessage = await isComponentVisible(page, 'ErrorMessage')

    if (hasErrorMessage) {
      const errorMessage = getComponent(page, 'ErrorMessage')
      const text = await errorMessage.textContent()
      console.log('Error:', text)
    }

    // Wait for a component with custom timeout
    await waitForComponent(page, 'LoadingSpinner', { timeout: 5000 })
  })

  test('example: combining with other selectors', async ({ page }) => {
    // Get the main component
    const usersSettings = getComponent(page, 'UsersSettings')

    // Use Playwright's role-based selectors within the component
    const createButton = usersSettings.getByRole('button', { name: 'Create' })
    await createButton.click()

    // Use test IDs if needed (for specific interactive elements)
    const deleteButton = usersSettings.locator('[data-testid="delete-user-123"]')
    await deleteButton.click()

    // Combine component selector with CSS selectors
    const activeUsers = usersSettings.locator('.user-item.active')
    await expect(activeUsers).toHaveCount(5)
  })

  test('example: navigating component hierarchy', async ({ page }) => {
    // Start with a page component
    const settingsPage = getComponent(page, 'UsersSettings')

    // Get a child component
    const usersList = getComponentWithin(settingsPage, 'UsersList')

    // Get components within that child
    const listItems = usersList.locator('[data-component-name="UserListItem"]')

    // Interact with nested components
    const firstItem = listItems.first()
    const editButton = firstItem.getByRole('button', { name: 'Edit' })
    await editButton.click()

    // Now we're in the edit drawer
    const editDrawer = await waitForComponent(page, 'EditUserDrawer')
    await expect(editDrawer).toBeVisible()
  })

  test('example: debugging components', async ({ page }) => {
    // Get all components on the page
    const allComponents = page.locator('[data-component-name]')

    // Log all component names (useful for debugging)
    const count = await allComponents.count()
    console.log(`Found ${count} components on page`)

    for (let i = 0; i < Math.min(count, 10); i++) {
      const name = await allComponents.nth(i).getAttribute('data-component-name')
      console.log(`Component ${i}: ${name}`)
    }
  })
})

/**
 * Tips for using component selectors:
 *
 * 1. Use component selectors for high-level component identification
 * 2. Use role-based selectors for interactive elements (buttons, inputs, etc.)
 * 3. Combine both for the most maintainable tests
 * 4. Component selectors are stable - they don't break when CSS changes
 * 5. They're semantic - test reads like "get the UsersList component"
 *
 * Example of good combination:
 *
 * ```typescript
 * // Component selector: identify which component
 * const createDrawer = getComponent(page, 'CreateUserDrawer')
 *
 * // Role selector: identify interactive elements within
 * await createDrawer.getByLabel('Username').fill('john')
 * await createDrawer.getByRole('button', { name: 'Submit' }).click()
 * ```
 */
