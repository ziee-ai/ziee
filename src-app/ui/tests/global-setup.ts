import { FullConfig } from '@playwright/test'
import { execSync } from 'child_process'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'fs'
import { tmpdir } from 'os'
import crypto from 'crypto'
import pg from 'pg'
import dotenv from 'dotenv'
import { cleanupStaleLocks, allocatePostgresPort } from './fixtures/port-manager'

const { Pool } = pg
const __dirname = dirname(fileURLToPath(import.meta.url))

export default async function globalSetup(_config: FullConfig) {
  // Load environment variables from .env.test
  dotenv.config({ path: resolve(__dirname, '.env.test') })

  console.log('\n🚀 Starting Playwright E2E Test Infrastructure...\n')

  // Clean up stale port locks from previous crashed/killed test runs
  cleanupStaleLocks()

  // Clean up any stale PostgreSQL test containers
  // Only remove containers whose lock files are missing or stale
  console.log('🧹 Cleaning up stale PostgreSQL containers...')
  try {
    const containers = execSync('docker ps -a --filter "name=ziee-test-postgres-" --format "{{.Names}}"', {
      encoding: 'utf-8',
    }).trim()

    if (containers) {
      const containerList = containers.split('\n')
      let removed = 0
      let kept = 0

      for (const container of containerList) {
        // Extract run ID from container name: ziee-test-postgres-{runId}
        const runId = container.replace('ziee-test-postgres-', '')
        const configPath = resolve(__dirname, `.test-configs/postgres-${runId}.json`)

        // Check if config file exists
        if (existsSync(configPath)) {
          // Config exists - check if lock is valid by reading the PID
          try {
            const config = JSON.parse(readFileSync(configPath, 'utf-8'))
            const lockFile = resolve(tmpdir(), 'ziee-test-locks', `postgres-${config.port}.lock`)

            if (existsSync(lockFile)) {
              const lock = JSON.parse(readFileSync(lockFile, 'utf-8'))
              // Check if process is still running
              try {
                process.kill(lock.pid, 0) // Signal 0 just checks if process exists
                console.log(`   ✅ Kept active container: ${container} (PID ${lock.pid})`)
                kept++
                continue
              } catch {
                // Process not running - lock is stale
              }
            }
          } catch {
            // Error reading config/lock - treat as stale
          }
        }

        // If we get here, container is stale (no config, no lock, or process dead)
        console.log(`   🗑️  Removing stale container: ${container}`)
        execSync(`docker rm -f ${container}`, { stdio: 'ignore' })
        removed++
      }

      if (removed > 0 || kept > 0) {
        console.log(`✅ Container cleanup: ${removed} removed, ${kept} kept\n`)
      } else {
        console.log('✅ No stale containers found\n')
      }
    } else {
      console.log('✅ No stale containers found\n')
    }
  } catch (error) {
    console.log('✅ No stale containers found\n')
  }

  // 1. Generate unique test run ID
  const runId = crypto.randomBytes(4).toString('hex')
  console.log(`🆔 Test run ID: ${runId}`)

  // 2. Allocate PostgreSQL port with file lock
  console.log('🔍 Allocating PostgreSQL port...')
  const postgresPort = await allocatePostgresPort(runId)
  console.log(`✅ Allocated PostgreSQL port: ${postgresPort}\n`)

  // 3. Create .test-configs directory if it doesn't exist
  const configDir = resolve(__dirname, '.test-configs')
  if (!existsSync(configDir)) {
    mkdirSync(configDir, { recursive: true })
  }

  // 4. Generate docker-compose.yaml from template
  console.log('📝 Generating docker-compose configuration...')
  const templatePath = resolve(__dirname, 'docker-compose-test-template.yaml')
  const dockerComposeContent = readFileSync(templatePath, 'utf-8')
    .replace(/\$\{RUN_ID\}/g, runId)
    .replace(/\$\{POSTGRES_PORT\}/g, postgresPort.toString())

  const dockerComposePath = resolve(configDir, `docker-compose-${runId}.yaml`)
  writeFileSync(dockerComposePath, dockerComposeContent)

  // 5. Store config for test-context.ts and global-teardown.ts
  const configData = {
    runId,
    port: postgresPort,
    dockerComposePath,
  }
  const configPath = resolve(configDir, `postgres-${runId}.json`)
  writeFileSync(configPath, JSON.stringify(configData, null, 2))

  // Store runId in environment for teardown
  process.env.TEST_RUN_ID = runId

  // 6. Start Docker PostgreSQL for this test run
  console.log(`🐘 Starting PostgreSQL container for run ${runId}...`)
  try {
    execSync(`docker compose -f "${dockerComposePath}" up -d`, {
      stdio: 'inherit',
    })
  } catch (error) {
    console.error('❌ Failed to start PostgreSQL container')
    throw error
  }

  // Wait for PostgreSQL to be fully ready
  console.log('⏳ Waiting for PostgreSQL to be ready...')
  await new Promise(resolve => setTimeout(resolve, 3000))

  // 7. Verify PostgreSQL connection
  const pool = new Pool({
    host: 'localhost',
    port: postgresPort,
    user: 'postgres',
    password: 'password',
    database: 'postgres',
  })

  let retries = 30
  while (retries > 0) {
    try {
      await pool.query('SELECT 1')
      console.log('✅ Connected to test PostgreSQL\n')
      break
    } catch (error) {
      retries--
      if (retries === 0) {
        console.error('❌ Failed to connect to PostgreSQL after 30 attempts:', error)
        await pool.end()
        throw error
      }
      await new Promise(resolve => setTimeout(resolve, 1000))
    }
  }
  await pool.end()

  console.log('✅ PostgreSQL ready for tests!\n')
  console.log('   Test infrastructure:')
  console.log(`   - PostgreSQL: port ${postgresPort} (container: ziee-test-postgres-${runId})`)
  console.log('   - Each worker: 2 dynamic ports (vite + backend)')
  console.log('   - Each test: unique database + backend restart')
  console.log('   - Worker 0: 9000+9100, Worker 1: 9001+9101, etc.\n')
}
