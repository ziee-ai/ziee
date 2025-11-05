# Playwright E2E Testing

This directory contains end-to-end tests for the Ziee Chat UI using Playwright.

## Architecture

- **Shared PostgreSQL**: Docker container on port 54320
- **Parallel Workers**: Each worker gets its own backend server and database
- **Isolated Tests**: Tests run in complete isolation with fresh data
- **Accessibility**: Every test includes accessibility checks with axe-core

## Prerequisites

- Node.js and npm installed
- Docker installed and running
- Rust and Cargo (for building backend servers)

## Setup

1. Install dependencies:
```bash
cd ui
npm install
```

2. Install Playwright browsers:
```bash
npx playwright install
```

## Running Tests

### Run all tests
```bash
npm run test:e2e
```

### Run tests with UI (interactive mode)
```bash
npm run test:e2e:ui
```

### Run tests in headed mode (see browser)
```bash
npm run test:e2e:headed
```

### Debug tests
```bash
npm run test:e2e:debug
```

### Run only chromium tests
```bash
npm run test:e2e:chromium
```

### View test report
```bash
npm run test:e2e:report
```

## Database Management

### Start PostgreSQL
```bash
npm run test:db:start
```

### Stop PostgreSQL
```bash
npm run test:db:stop
```

### Clean PostgreSQL (remove all data)
```bash
npm run test:db:clean
```

### View PostgreSQL logs
```bash
npm run test:db:logs
```

## Test Structure

```
tests/
├── e2e/                    # Test files
│   └── 01-setup/
│       └── setup.spec.ts   # Setup flow tests
├── fixtures/               # Test fixtures
│   └── worker-context.ts   # Worker-specific context
├── utils/                  # Test utilities
│   ├── accessibility.ts    # Accessibility helpers
│   ├── auth.ts            # Authentication helpers
│   ├── database.ts        # Database helpers
│   └── user-management.ts # User management helpers
├── global-setup.ts        # Global test setup
├── global-teardown.ts     # Global test cleanup
└── README.md             # This file
```

## Writing Tests

### Basic Test Structure

```typescript
import { test, expect } from '../../fixtures/worker-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'

test.describe('Feature Name', () => {
  test('should do something', async ({ page, baseURL, apiURL }) => {
    // Navigate to page
    await page.goto(`${baseURL}/path`)

    // Test functionality
    await page.fill('input[name="field"]', 'value')
    await page.click('button[type="submit"]')

    // Assert results
    await expect(page).toHaveURL(`${baseURL}/success`)

    // ALWAYS check accessibility
    await assertNoAccessibilityViolations(page)
  })
})
```

### Using Database Helpers

```typescript
import { getDatabasePool } from '../../utils/database'
import { createAdminUser } from '../../utils/user-management'

test('should work with admin user', async ({ page, baseURL, workerInfo }) => {
  // Get database pool for this worker
  const pool = await getDatabasePool(workerInfo.workerId)

  // Create admin user
  await createAdminUser(pool, 'admin', 'password123')

  // Run test...

  // Cleanup
  await pool.end()
})
```

### Using Auth Helpers

```typescript
import { loginViaUI, registerUser } from '../../utils/auth'

test('should login successfully', async ({ page, baseURL, apiURL }) => {
  // Register user via API
  const { token, userId } = await registerUser(apiURL, {
    username: 'testuser',
    email: 'test@example.com',
    password: 'password123',
  })

  // Login via UI
  await loginViaUI(page, baseURL, 'testuser', 'password123')

  // User should be logged in...
})
```

## Accessibility Testing

**EVERY test MUST include accessibility checks.**

```typescript
import { assertNoAccessibilityViolations } from '../../utils/accessibility'

test('my test', async ({ page, baseURL }) => {
  await page.goto(`${baseURL}/page`)

  // ... test functionality ...

  // REQUIRED: Check accessibility
  await assertNoAccessibilityViolations(page)
})
```

### Custom Accessibility Options

```typescript
await assertNoAccessibilityViolations(page, {
  // Only test specific WCAG levels
  tags: ['wcag2a', 'wcag2aa'],

  // Disable specific rules
  disabledRules: ['color-contrast'],

  // Only check specific elements
  include: ['.main-content'],

  // Exclude elements
  exclude: ['.third-party-widget'],
})
```

## How It Works

### Global Setup (runs once before all tests)

1. Starts Docker PostgreSQL container
2. Creates N databases (one per worker)
3. Generates N config files for backend servers
4. Starts N backend servers (cargo run with different ports)
5. Waits for all servers to be healthy
6. Saves worker info to `.test-workers.json`

### Per-Worker Setup (runs once per worker)

- Each worker reads `.test-workers.json` to get its backend server URL
- Tests use `baseURL` and `apiURL` from worker context

### Per-Test (runs for each test)

- Tests have isolated database via their worker
- Can use database helpers to seed/clean data
- Must check accessibility

### Global Teardown (runs once after all tests)

1. Kills all backend servers
2. Stops Docker PostgreSQL
3. Cleans up temp config files

## Troubleshooting

### Tests fail to start

Check Docker is running:
```bash
docker ps
```

### Backend servers fail to start

Build the backend first:
```bash
cd ../src-web
cargo build
```

### PostgreSQL port conflict

Check if port 54320 is already in use:
```bash
lsof -i :54320
```

Clean up and restart:
```bash
npm run test:db:clean
npm run test:db:start
```

### Accessibility violations

When tests fail with accessibility violations:

1. Read the violation details in the error message
2. Fix the component code
3. Re-run the test to verify

Example violation:
```
❌ button-name: Buttons must have discernible text
   Impact: critical
   Help: Buttons must have discernible text
   Learn more: https://dequeuniversity.com/rules/axe/4.10/button-name
   Affected elements:
     - <button type="button"><svg>...</svg></button>
```

Fix:
```tsx
// Before (wrong):
<Button icon={<PlusOutlined />} />

// After (correct):
<Button icon={<PlusOutlined />} aria-label="Add item" />
```

## CI/CD Integration

Tests can run in CI with:

```yaml
- name: Run E2E Tests
  run: |
    cd ui
    npm install
    npx playwright install --with-deps
    npm run test:e2e
```

## Resources

- [Playwright Documentation](https://playwright.dev/)
- [axe-core Rules](https://github.com/dequelabs/axe-core/blob/develop/doc/rule-descriptions.md)
- [WCAG 2.1 Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
