export default async function globalTeardown() {
  console.log('\n🧹 Cleaning up desktop test infrastructure...\n')

  // Get test run ID from environment
  const runId = process.env.TEST_RUN_ID
  if (!runId) {
    console.log('⚠️  No test run ID found')
    return
  }

  // Note: The webServer config in playwright.config.ts handles stopping the dev server
  // Additional cleanup can be added here if needed

  console.log('✅ Desktop test cleanup complete!\n')
}
