# User Module E2E Tests

## Overview

Comprehensive end-to-end tests for the user management module, covering users and groups functionality.

## Test Structure

### Test Files

1. **01-users-crud.spec.ts** (12 tests)
   - User creation with/without permissions
   - User editing
   - User deletion
   - Form validation
   - Duplicate username handling
   - Pagination

2. **02-groups-crud.spec.ts** (15 tests)
   - Group creation with/without permissions
   - Group editing
   - Group deletion
   - Group status toggle (active/inactive)
   - System group protection
   - Form validation
   - Pagination

3. **03-user-status.spec.ts** (7 tests)
   - Toggle user status (active/inactive)
   - Password reset
   - Status confirmation dialogs
   - Validation for password length
   - Admin user protection

4. **04-group-members.spec.ts** (9 tests)
   - View group members
   - Display member information
   - Empty state handling
   - User-group navigation

### Helper Modules

- **user-navigation.ts**: Navigation helpers for pages and drawers
- **user-actions.ts**: User CRUD operations and status management
- **group-actions.ts**: Group CRUD operations and membership management
- **user-assertions.ts**: Assertion helpers for UI state verification

## Test Coverage

- ✅ User CRUD operations
- ✅ Group CRUD operations
- ✅ User status management (active/inactive)
- ✅ Password reset functionality
- ✅ Group membership viewing
- ✅ Form validation
- ✅ Error handling
- ✅ Pagination
- ✅ System entity protection (admin user, system groups)
- ✅ Empty state handling

## Known Issues

### Authentication Required

All tests require admin authentication before accessing user management pages. The tests need to be updated to:

1. Import from test context fixture:
   ```typescript
   import { test, expect } from '../../fixtures/test-context'
   import { loginAsAdmin } from '../../common/auth-helpers'
   ```

2. Use `testInfra` fixture for `baseURL`:
   ```typescript
   test('example test', async ({ page, testInfra }) => {
     const { baseURL } = testInfra
     await loginAsAdmin(page, baseURL)
     await navigateToUsers(page, baseURL)
     // ...
   })
   ```

3. Update navigation helpers to accept `baseURL` parameter (✅ DONE)

## Running the Tests

```bash
# Run all user module tests
npm run test:e2e -- tests/e2e/02-users/

# Run specific test file
npm run test:e2e -- tests/e2e/02-users/01-users-crud.spec.ts

# Run with UI mode for debugging
npm run test:e2e -- tests/e2e/02-users/ --ui
```

## Notes

- Tests use semantic selectors (getByRole, getByLabel) following accessibility best practices
- Unique timestamps are used for test data to avoid conflicts
- Tests clean up after themselves by deleting created entities
- Pagination tests are conditional (only run if pagination exists)
- System entity tests are conditional (only run if system entities exist)
