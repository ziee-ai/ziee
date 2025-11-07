# Component Selectors for Testing

## Overview

All React components automatically receive a `data-component-name` attribute in development and test environments. This provides stable, semantic selectors for E2E tests.

## How It Works

### Automatic Transformation

The Babel plugin (`babel-plugin-add-component-name.cjs`) automatically adds `data-component-name` attributes to all function components:

**Before (your code):**
```tsx
export function UsersList() {
  return (
    <div className="users-list">
      <h2>Users</h2>
      {/* ... */}
    </div>
  )
}
```

**After (in browser during dev/test):**
```tsx
export function UsersList() {
  return (
    <div className="users-list" data-component-name="UsersList">
      <h2>Users</h2>
      {/* ... */}
    </div>
  )
}
```

### When It's Active

- ✅ **Development mode** (`NODE_ENV !== 'production'`)
- ✅ **Test mode** (`NODE_ENV === 'test'`)
- ❌ **Production builds** (attributes are not added to keep bundles clean)

## Usage in Tests

### Basic Usage

```typescript
import { test, expect } from '@playwright/test'
import { getComponent, waitForComponent } from '../helpers/component-selectors'

test('should display users list', async ({ page }) => {
  await page.goto('/settings/users')

  // Wait for component to appear
  await waitForComponent(page, 'UsersList')

  // Get component and assert
  const usersList = getComponent(page, 'UsersList')
  await expect(usersList).toBeVisible()
})
```

### Working with Drawers/Modals

```typescript
test('should open create user drawer', async ({ page }) => {
  await page.goto('/settings/users')

  // Click create button
  await clickComponent(page, 'CreateUserButton')

  // Wait for drawer to open
  const drawer = await waitForComponent(page, 'CreateUserDrawer')

  // Interact with drawer components
  const usernameInput = getComponentWithin(drawer, 'UsernameInput')
  await usernameInput.fill('john.doe')

  await clickComponent(page, 'SubmitButton')
})
```

### Checking Multiple Instances

```typescript
test('should render all user list items', async ({ page }) => {
  await page.goto('/settings/users')

  // Get all instances of a component
  const listItems = getAllComponents(page, 'UserListItem')

  // Check count
  await expect(listItems).toHaveCount(10)

  // Check first item
  await expect(listItems.first()).toBeVisible()
})
```

### Conditional Checks

```typescript
test('should show error message on failure', async ({ page }) => {
  await page.goto('/settings/users')

  // Trigger an error
  await clickComponent(page, 'DeleteButton')

  // Check if error component is visible
  if (await isComponentVisible(page, 'ErrorMessage')) {
    const errorText = await getComponentText(page, 'ErrorMessage')
    expect(errorText).toContain('Failed to delete')
  }
})
```

## Helper Functions

All helper functions are available in `tests/helpers/component-selectors.ts`:

### `getComponent(page, componentName)`
Get a component by its name.

### `getAllComponents(page, componentName)`
Get all instances of a component (useful for lists).

### `getComponentWithin(parent, componentName)`
Get a component scoped within a parent locator.

### `waitForComponent(page, componentName, options?)`
Wait for a component to be visible.

### `isComponentVisible(page, componentName)`
Check if a component is visible.

### `clickComponent(page, componentName, options?)`
Click on a component.

### `getComponentText(page, componentName)`
Get component text content.

## Best Practices

### 1. Use Semantic Component Names

Name your components descriptively:

```tsx
// ✅ Good
export function UserListItem() { ... }
export function CreateUserDrawer() { ... }
export function SubmitButton() { ... }

// ❌ Avoid
export function Item() { ... }
export function Drawer() { ... }
export function Button1() { ... }
```

### 2. Combine with Other Selectors

Component selectors work great with Playwright's other selectors:

```typescript
// Find button within UsersList component
const deleteButton = getComponent(page, 'UsersList')
  .getByRole('button', { name: 'Delete' })

// Find input by label within drawer
const drawer = getComponent(page, 'CreateUserDrawer')
const emailInput = drawer.getByLabel('Email')
```

### 3. Prefer Component Selectors Over CSS Classes

```typescript
// ✅ Better - stable, semantic
const usersList = getComponent(page, 'UsersList')

// ❌ Avoid - fragile, implementation detail
const usersList = page.locator('.users-list-container-wrapper')
```

### 4. Use for Component-Level Testing

Component selectors are perfect for testing at the component level:

```typescript
test('UsersList component', async ({ page }) => {
  // Test the UsersList component specifically
  const usersList = getComponent(page, 'UsersList')

  // Test its children
  const header = usersList.getByRole('heading')
  const list = usersList.getByRole('list')

  await expect(header).toHaveText('Users')
  await expect(list).toBeVisible()
})
```

## Troubleshooting

### Attribute Not Appearing

If you don't see the `data-component-name` attribute:

1. **Check environment**: Make sure `NODE_ENV !== 'production'`
2. **Check component syntax**: Must be a function component (not class component)
3. **Check component name**: Component must start with uppercase letter
4. **Restart dev server**: Changes to Babel config require restart

### Multiple Components with Same Name

If you have multiple components with the same name in different files:

```typescript
// Use nth() to select specific instance
const firstModal = getComponent(page, 'Modal').nth(0)
const secondModal = getComponent(page, 'Modal').nth(1)

// Or use additional context
const userModal = page.locator('#user-section')
  .locator('[data-component-name="Modal"]')
```

### Component Not Found

```typescript
// Check if component exists before interacting
const component = getComponent(page, 'MyComponent')
if (await component.count() > 0) {
  await component.click()
} else {
  console.log('Component not found')
}
```

## Debugging

### View All Components

```typescript
test('debug: list all components', async ({ page }) => {
  await page.goto('/settings/users')

  // Get all elements with data-component-name
  const components = await page.locator('[data-component-name]').all()

  for (const component of components) {
    const name = await component.getAttribute('data-component-name')
    console.log('Component:', name)
  }
})
```

### Inspect in Browser

In dev mode, open DevTools and run:

```javascript
// List all component names
document.querySelectorAll('[data-component-name]').forEach(el => {
  console.log(el.getAttribute('data-component-name'))
})

// Find specific component
document.querySelector('[data-component-name="UsersList"]')
```

## Examples from Codebase

### User Settings Page

```typescript
test('should manage users', async ({ page }) => {
  await page.goto('/settings/users')

  // Main settings page
  const usersSettings = getComponent(page, 'UsersSettings')
  await expect(usersSettings).toBeVisible()

  // List of users
  const usersList = getComponentWithin(usersSettings, 'UsersList')
  await expect(usersList).toBeVisible()

  // Open create drawer
  await clickComponent(page, 'CreateUserDrawer')

  // Fill form in drawer
  const drawer = getComponent(page, 'CreateUserDrawer')
  await drawer.getByLabel('Username').fill('john')
  await drawer.getByLabel('Email').fill('john@example.com')

  // Submit
  await drawer.getByRole('button', { name: 'Create User' }).click()
})
```

## Migration Guide

### Before (Traditional Selectors)

```typescript
// Using CSS classes
await page.locator('.ant-drawer:visible .ant-drawer-title:has-text("Create User")')

// Using text content
await page.locator('text=Create User').click()

// Using roles (still good!)
await page.getByRole('button', { name: 'Create User' })
```

### After (Component Selectors)

```typescript
// Using component name
await getComponent(page, 'CreateUserDrawer')

// Still use text for actions
await clickComponent(page, 'CreateButton')

// Combine with roles
await getComponent(page, 'CreateUserDrawer')
  .getByRole('button', { name: 'Submit' })
```

## Related Files

- **Plugin**: `babel-plugin-add-component-name.cjs`
- **Config**: `vite.config.ts`
- **Helpers**: `tests/helpers/component-selectors.ts`
- **Examples**: `tests/e2e/**/*.spec.ts`
