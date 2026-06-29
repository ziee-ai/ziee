import { defineConfig, devices } from '@playwright/test'
import crypto from 'crypto'

// Organize test results by test run ID to avoid conflicts between parallel test runs
// Generate a unique ID for this test run if not set by global-setup
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
  // Each test gets its own worker with isolated backend server, Vite server, and database
  fullyParallel: true,

  // Fail the build on CI if you accidentally left test.only
  forbidOnly: !!process.env.CI,

  // Retry on CI only
  retries: process.env.CI ? 2 : 0,

  // Each test spins up its OWN full stack (a fresh `cargo run` backend + Vite +
  // Postgres). High parallelism therefore saturates the CPU while many servers
  // cold-boot at once, which widens the per-test readiness window and triggers
  // SSE/connection churn — the dominant source of e2e flakiness. Default to 1
  // (the validated-stable value, paired with the deep readiness gate in
  // tests/fixtures/test-context.ts); raise via PLAYWRIGHT_WORKERS on beefier CI
  // once a higher count is validated.
  workers: process.env.PLAYWRIGHT_WORKERS
    ? Number(process.env.PLAYWRIGHT_WORKERS)
    : 1,

  // Reporter to use
  reporter: [
    ['html', { outputFolder: reportDir, open: 'never' }],
    ['junit', { outputFile: `${reportDir}/results.xml` }],
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

  // Dependency DAG / ordering
  // ---------------------------
  // Test dirs are SEMANTICALLY named (setup/ auth/ llm/ chat/ ...); the old
  // numeric prefixes (01-/02-/...) were cosmetic — Playwright never enforced
  // them as order. The real ordering need is a DAG, encoded here, NOT in
  // filenames (see .claude/audit/TEST_CONVENTIONS.md).
  //
  // Because every TEST is fully self-isolated — each one CREATEs its own
  // `ziee_test_<id>` database + spawns its own `cargo run` backend on
  // per-worker ports (tests/fixtures/test-context.ts) and seeds its own state
  // via loginAsAdmin/setup — there are NO inter-suite state dependencies. The
  // only global prerequisite (the shared Postgres container + the one-time
  // `vite build`) runs in `globalSetup` BEFORE any project, which is the single
  // root of the DAG. Adding setup→auth→features project edges would serialize
  // independent suites and destroy the parallelism the per-test isolation
  // buys, so the DAG is intentionally flat:
  //
  //   globalSetup  ──▶  { all feature suites, run fully in parallel }
  //
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
