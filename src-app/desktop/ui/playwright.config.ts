import { defineConfig, devices } from '@playwright/test'
import crypto from 'crypto'

// Organize test results by test run ID to avoid conflicts between parallel test runs
const testRunId = process.env.TEST_RUN_ID || crypto.randomBytes(4).toString('hex')
const outputDir = `test-results/${testRunId}`
const reportDir = `playwright-report/${testRunId}`

// Make test run ID available to global-setup
if (!process.env.TEST_RUN_ID) {
  process.env.TEST_RUN_ID = testRunId
}

export default defineConfig({
  testDir: './tests/e2e',

  // Organize test artifacts by test run ID
  outputDir,

  // Run tests in files in parallel
  fullyParallel: true,

  // Fail the build on CI if you accidentally left test.only
  forbidOnly: !!process.env.CI,

  // Retry on CI only
  retries: process.env.CI ? 2 : 0,

  // Desktop tests run with fewer workers since Tauri manages backend
  workers: 4,

  // Reporter to use
  reporter: [
    ['html', { outputFolder: reportDir, open: 'never' }],
    ['junit', { outputFile: `${reportDir}/results.xml` }],
    ['list'],
  ],

  // Shared settings for all projects
  use: {
    // Base URL for desktop app's Vite dev server
    baseURL: 'http://localhost:1420',

    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',

    // Timeout for actions (clicks, fills, etc.)
    actionTimeout: 10000,
  },

  // Global setup and teardown
  globalSetup: './tests/global-setup.ts',
  globalTeardown: './tests/global-teardown.ts',

  // Configure projects for major browsers
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1280, height: 720 },
      },
    },
  ],

  // Global timeout for each test
  timeout: 120000, // 2 minutes per test

  // Timeout for expect() assertions
  expect: {
    timeout: 10000,
  },

  // Dev server configuration - start Tauri dev server for tests
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 120000, // 2 minutes for server to start
  },
})
