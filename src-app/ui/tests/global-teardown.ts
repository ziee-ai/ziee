import { execSync } from 'child_process'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import { readFileSync, rmSync, existsSync } from 'fs'
import { releasePostgresPortLock } from './fixtures/port-manager'

const __dirname = dirname(fileURLToPath(import.meta.url))

export default async function globalTeardown() {
  console.log('\n🧹 Cleaning up test infrastructure...\n')

  // Get test run ID from environment
  const runId = process.env.TEST_RUN_ID
  if (!runId) {
    console.log('⚠️  No test run ID found, skipping cleanup')
    return
  }

  // Read config file
  const configDir = resolve(__dirname, '.test-configs')
  const configPath = resolve(configDir, `postgres-${runId}.json`)

  if (!existsSync(configPath)) {
    console.log('⚠️  Config file not found, skipping cleanup')
    return
  }

  try {
    const config = JSON.parse(readFileSync(configPath, 'utf-8'))
    const { port, dockerComposePath } = config

    // Stop and remove Docker PostgreSQL container with volumes
    console.log(`🛑 Stopping PostgreSQL container for run ${runId}...`)
    try {
      execSync(`docker compose -f "${dockerComposePath}" down -v`, {
        stdio: 'inherit',
      })
      console.log('✅ PostgreSQL container stopped')
    } catch (error) {
      console.error('❌ Failed to stop PostgreSQL container:', error)
    }

    // Release port lock
    console.log(`🔓 Releasing PostgreSQL port lock...`)
    releasePostgresPortLock(port)

    // Clean up config files
    console.log(`🗑️  Removing configuration files...`)
    try {
      rmSync(dockerComposePath, { force: true })
      rmSync(configPath, { force: true })
      console.log('✅ Configuration files removed')
    } catch (error) {
      console.error('❌ Failed to remove config files:', error)
    }
  } catch (error) {
    console.error('❌ Error during cleanup:', error)
  }

  console.log('\n✅ Cleanup complete!\n')
}
