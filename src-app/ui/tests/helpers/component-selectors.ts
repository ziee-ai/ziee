/**
 * Test helper functions for selecting components by their data-component-name attribute
 *
 * The Babel plugin automatically adds data-component-name attributes to all React components
 * in development and test environments, making it easy to write stable, semantic selectors.
 */

import { type Page, type Locator } from '@playwright/test'

/**
 * Get a component by its name
 *
 * @example
 * const userList = getComponent(page, 'UsersList')
 * await expect(userList).toBeVisible()
 */
export function getComponent(page: Page, componentName: string): Locator {
  return page.locator(`[data-component-name="${componentName}"]`)
}

/**
 * Get all instances of a component by name
 * Useful when a component is rendered multiple times (e.g., list items)
 *
 * @example
 * const listItems = getAllComponents(page, 'UserListItem')
 * await expect(listItems).toHaveCount(5)
 */
export function getAllComponents(page: Page, componentName: string): Locator {
  return page.locator(`[data-component-name="${componentName}"]`)
}

/**
 * Get a component scoped within a parent locator
 *
 * @example
 * const modal = getComponent(page, 'CreateUserDrawer')
 * const submitButton = getComponentWithin(modal, 'SubmitButton')
 */
export function getComponentWithin(parent: Locator, componentName: string): Locator {
  return parent.locator(`[data-component-name="${componentName}"]`)
}

/**
 * Wait for a component to be visible
 *
 * @example
 * await waitForComponent(page, 'UsersList')
 */
export async function waitForComponent(
  page: Page,
  componentName: string,
  options?: { timeout?: number }
): Promise<Locator> {
  const component = getComponent(page, componentName)
  await component.waitFor({ state: 'visible', ...options })
  return component
}

/**
 * Check if a component is visible
 *
 * @example
 * if (await isComponentVisible(page, 'ErrorMessage')) {
 *   // Handle error
 * }
 */
export async function isComponentVisible(
  page: Page,
  componentName: string
): Promise<boolean> {
  return await getComponent(page, componentName).isVisible()
}

/**
 * Click on a component
 *
 * @example
 * await clickComponent(page, 'CreateUserButton')
 */
export async function clickComponent(
  page: Page,
  componentName: string,
  options?: { timeout?: number }
): Promise<void> {
  await getComponent(page, componentName).click(options)
}

/**
 * Get component text content
 *
 * @example
 * const userName = await getComponentText(page, 'UserNameDisplay')
 */
export async function getComponentText(
  page: Page,
  componentName: string
): Promise<string | null> {
  return await getComponent(page, componentName).textContent()
}
