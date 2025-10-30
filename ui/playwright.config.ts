import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './tests/e2e',

  // Run tests in files in parallel
  // Each test gets its own worker with isolated backend server, Vite server, and database
  fullyParallel: true,

  // Fail the build on CI if you accidentally left test.only
  forbidOnly: !!process.env.CI,

  // Retry on CI only
  retries: process.env.CI ? 2 : 0,

  // Each test gets its own isolated infrastructure (backend + Vite + database)
  workers: 8,

  // Reporter to use
  reporter: [
    ['html', { outputFolder: 'playwright-report', open: 'never' }],
    ['junit', { outputFile: 'playwright-report/results.xml' }],
    ['list'],
  ],

  // Shared settings for all projects
  use: {
    // Base URL will be set per worker in fixture
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
    // Uncomment to test on other browsers
    // {
    //   name: 'firefox',
    //   use: {
    //     ...devices['Desktop Firefox'],
    //     viewport: { width: 1280, height: 720 },
    //   },
    // },
    // {
    //   name: 'webkit',
    //   use: {
    //     ...devices['Desktop Safari'],
    //     viewport: { width: 1280, height: 720 },
    //   },
    // },

    // Mobile viewports
    // {
    //   name: 'mobile-chrome',
    //   use: {
    //     ...devices['Pixel 5'],
    //   },
    // },
    // {
    //   name: 'mobile-safari',
    //   use: {
    //     ...devices['iPhone 12'],
    //   },
    // },
  ],

  // Global timeout for each test (includes infrastructure setup time)
  timeout: 180000, // 3 minutes per test (allows time for backend compilation + setup)

  // Timeout for expect() assertions
  expect: {
    timeout: 10000,
  },
})
