import { FullConfig } from '@playwright/test'
import crypto from 'crypto'

export default async function globalSetup(_config: FullConfig) {
  console.log('\n🚀 Starting Desktop E2E Test Infrastructure...\n')

  // Generate unique test run ID
  const runId = process.env.TEST_RUN_ID || crypto.randomBytes(4).toString('hex')
  console.log(`🆔 Test run ID: ${runId}`)
  process.env.TEST_RUN_ID = runId

  // Note: The webServer config in playwright.config.ts handles starting the dev server
  // The dev server starts both:
  // 1. Vite on port 1420 (serves the frontend)
  // 2. Proxies /api/ requests to the backend (localhost:3000)

  // For full E2E tests with backend, ensure the backend server is running
  // You can either:
  // 1. Start the backend manually before running tests
  // 2. Use the Tauri dev command which starts both frontend and embedded backend
  // 3. Modify this setup to start the backend programmatically

  console.log('\n✅ Desktop test infrastructure ready!')
  console.log('   - Vite dev server: http://localhost:1420')
  console.log('   - API proxy: /api/ -> http://localhost:3000')
  console.log('   - For full E2E tests, ensure backend is running\n')
}
