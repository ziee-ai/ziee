# Test Helper Refactoring Needed

## Critical Anti-Patterns Found

### 1. `closeAnyOpenDrawers()` in model-helpers.ts - **MUST REMOVE**

**Location**: `tests/e2e/05-llm/helpers/model-helpers.ts:18-34`

**Problem**: This is explicitly called out in CLAUDE.md as an anti-pattern:
```typescript
// ❌ NEVER use generic cleanup helpers:
await closeAnyOpenDrawers(page)  // ❌ WRONG
```

**Why it's bad**:
- Tests must know exactly what UI state they create and clean up
- Generic helpers hide test dependencies
- Causes flaky tests

**Current usage**:
```typescript
export async function closeAnyOpenDrawers(page: Page): Promise<void> {
  // Attempts to close all visible drawers generically
  const closeButtons = page.locator('.ant-drawer:visible button[aria-label="Close drawer"]')
  // ...
}

// Used in:
async function openAddModelDropdown(page: Page) {
  await closeAnyOpenDrawers(page)  // ❌ WRONG
  // ...
}

async function deleteModel(page: Page, modelName: string) {
  await closeAnyOpenDrawers(page)  // ❌ WRONG
  // ...
}
```

**Solution**: Each test should explicitly close the drawer it opened:
```typescript
// ✅ CORRECT - explicitly close the drawer you opened
test('should upload model', async ({ page }) => {
  await openUploadDrawer(page)
  await fillUploadForm(page, data)
  await submitUploadForm(page)

  // Explicitly close the drawer we opened
  const uploadDrawer = page.getByRole('dialog', { name: 'Upload Local Model' })
  await uploadDrawer.getByRole('button', { name: 'Cancel' }).click()
  await uploadDrawer.waitFor({ state: 'hidden' })
})
```

---

## Form Field ID Anti-Patterns

### 2. Auth form-helpers.ts

**Problem**: Using form field IDs instead of semantic selectors

```typescript
// ❌ WRONG
await page.fill('#login_username', username)
await page.fill('#login_password', password)
await page.fill('#register_email', email)

// ✅ CORRECT
await page.getByLabel('Username or Email').fill(username)
await page.getByLabel('Password').fill(password)
await page.getByLabel('Email').fill(email)
```

### 3. Auth navigation-helpers.ts

**Problem**: Using form field IDs for wait conditions

```typescript
// ❌ WRONG
await page.waitForSelector('#login_username', { timeout: 30000 })

// ✅ CORRECT
await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
```

### 4. Assistant-helpers.ts

**Problem**: Using form field IDs

```typescript
// ❌ WRONG
await page.fill('#assistant-form_name', data.name)
await page.fill('#assistant-form_description', data.description)
await page.locator('#assistant-form_enabled').click()

// ✅ CORRECT
await page.getByLabel('Name').fill(data.name)
await page.getByLabel('Description').fill(data.description)
await page.getByLabel('Enabled').click()
```

---

## CSS Class Selector Anti-Patterns

### 5. Model-helpers.ts

**Problems**:
```typescript
// ❌ Using CSS classes for structural elements
const addButton = page.locator('.ant-card-head:has-text("Models") button[data-icon="plus"]')
const dropdown = page.locator('.ant-dropdown-menu')
await page.waitForSelector('.ant-drawer-title:has-text("Upload Local Model")')

// ✅ BETTER - use semantic selectors where possible
const addButton = page.getByRole('button', { name: 'Add model' })
const dropdown = page.getByRole('menu')
await page.getByRole('dialog', { name: 'Upload Local Model' }).waitFor()
```

### 6. Assistant-helpers.ts

**Problems**:
```typescript
// ❌ Using CSS classes
await page.click('.ant-drawer button[type="submit"]')
const card = page.locator('.ant-card:has-text("${assistantName}")')

// ✅ BETTER
await page.getByRole('button', { name: 'Submit' }).click()
const card = page.getByText(assistantName).locator('..')  // or use data-test-id
```

---

## Recommendations

### Priority 1: Remove `closeAnyOpenDrawers()`
**Impact**: High - causes flaky tests
**Effort**: Medium - need to update tests to explicitly close drawers

### Priority 2: Refactor form field selectors
**Impact**: Medium - makes tests more resilient
**Effort**: Low - straightforward find/replace

### Priority 3: Replace CSS class selectors
**Impact**: Medium - improves maintainability
**Effort**: Medium - need to add ARIA labels to components

---

## Implementation Strategy

1. **Remove `closeAnyOpenDrawers()`**:
   - Find all usages in tests
   - Replace with explicit drawer closing
   - Use `afterEach` for cleanup if needed

2. **Add semantic selectors**:
   - Replace `#form_id` with `getByLabel('Label')`
   - Replace `button:has-text()` with `getByRole('button', { name })`
   - Replace `.ant-drawer` with `getByRole('dialog')`

3. **Use `data-test-*` sparingly**:
   - Only for elements that truly can't use semantic selectors
   - Prefer `data-test-id` over component-specific attributes
   - Remember: each `data-test-*` is a code smell

---

## Files to Refactor

- [ ] `tests/e2e/02-auth/helpers/form-helpers.ts`
- [ ] `tests/e2e/02-auth/helpers/navigation-helpers.ts`
- [ ] `tests/e2e/05-llm/helpers/model-helpers.ts` (PRIORITY 1)
- [ ] `tests/e2e/06-assistants/helpers/assistant-helpers.ts`
- [ ] Any tests that call `closeAnyOpenDrawers()`
