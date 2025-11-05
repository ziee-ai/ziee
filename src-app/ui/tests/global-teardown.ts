import { execSync } from 'child_process'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))

export default async function globalTeardown() {
  console.log('\n🧹 Cleaning up test infrastructure...\n')

  // Stop Docker PostgreSQL
  console.log('🛑 Stopping Docker PostgreSQL...')
  try {
    execSync('node scripts/test-db.js stop', {
      cwd: resolve(__dirname, '..'),
      stdio: 'inherit',
    })
  } catch (error) {
    console.error('❌ Failed to stop PostgreSQL:', error)
  }

  console.log('\n✅ Cleanup complete!\n')
}
