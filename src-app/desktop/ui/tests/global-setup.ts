/**
 * Playwright global-setup for desktop UI tests.
 *
 * Runs ONCE before any test. Stands up the shared PostgreSQL container
 * that every per-test backend will create its own database inside, and
 * cleans up any leftover state from crashed previous runs.
 *
 * Each test gets its own backend process (spawned in test-context.ts)
 * + its own DB inside this shared Postgres. Per-test isolation; one
 * Docker container to amortize the heavy startup.
 *
 * For tests that only need the Vite dev server (the mocked-backend
 * specs like desktop-auto-login / settings-filter), this setup still
 * runs but they ignore the Postgres infrastructure.
 */

import { FullConfig } from '@playwright/test'
import { execSync } from 'child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'fs'
import { tmpdir } from 'os'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import crypto from 'crypto'
import pg from 'pg'
import {
  allocatePostgresPort,
  cleanupStaleConfigFiles,
  cleanupStaleLocks,
} from './fixtures/port-manager'

const { Pool } = pg
const __dirname = dirname(fileURLToPath(import.meta.url))

export default async function globalSetup(_config: FullConfig) {
  console.log('\n🚀 Starting Desktop E2E Test Infrastructure...\n')

  // Ensure the Playwright browser binary is present (idempotent; fast when
  // already installed). Baked in so no manual `npx playwright install` is
  // needed for any desktop-e2e invocation.
  try {
    console.log('🌐 Ensuring Playwright chromium is installed...')
    execSync('npx playwright install chromium', { stdio: 'inherit' })
  } catch (e) {
    console.warn('⚠️  playwright install chromium failed (continuing):', e)
  }

  // Allocate a test run ID (used to namespace the Docker container +
  // config files) and stash it in env for teardown / test-context.
  const runId = process.env.TEST_RUN_ID || crypto.randomBytes(4).toString('hex')
  console.log(`🆔 Test run ID: ${runId}`)
  process.env.TEST_RUN_ID = runId

  // Cleanup stale state from previously-crashed runs.
  cleanupStaleLocks()
  const configDir = resolve(__dirname, '.test-configs')
  cleanupStaleConfigFiles(configDir)

  // Cleanup stale Docker test containers (containers whose lock files
  // are missing or whose owning PID is dead).
  console.log('🧹 Cleaning up stale PostgreSQL containers...')
  try {
    const containers = execSync(
      'docker ps -a --filter "name=ziee-desktop-test-postgres-" --format "{{.Names}}"',
      { encoding: 'utf-8' },
    ).trim()

    if (containers) {
      let removed = 0
      let kept = 0
      for (const container of containers.split('\n')) {
        const runIdFromName = container.replace(
          'ziee-desktop-test-postgres-',
          '',
        )
        const configPath = resolve(
          configDir,
          `postgres-${runIdFromName}.json`,
        )

        if (existsSync(configPath)) {
          try {
            const cfg = JSON.parse(readFileSync(configPath, 'utf-8'))
            const lockFile = resolve(
              tmpdir(),
              'ziee-desktop-test-locks',
              `postgres-${cfg.port}.lock`,
            )
            if (existsSync(lockFile)) {
              const lock = JSON.parse(readFileSync(lockFile, 'utf-8'))
              try {
                process.kill(lock.pid, 0)
                console.log(`   ✅ Kept active container: ${container}`)
                kept++
                continue
              } catch {
                // Owner PID gone — fall through to remove.
              }
            }
          } catch {
            // Corrupted config — fall through to remove.
          }
        }

        console.log(`   🗑️  Removing stale container: ${container}`)
        execSync(`docker rm -f ${container}`, { stdio: 'ignore' })
        removed++
      }
      console.log(`✅ Container cleanup: ${removed} removed, ${kept} kept\n`)
    } else {
      console.log('✅ No stale containers found\n')
    }
  } catch {
    console.log('✅ No stale containers found\n')
  }

  // Allocate a unique Postgres port and start a fresh container.
  console.log('🔍 Allocating PostgreSQL port...')
  const postgresPort = await allocatePostgresPort(runId)
  console.log(`✅ Allocated PostgreSQL port: ${postgresPort}\n`)

  if (!existsSync(configDir)) mkdirSync(configDir, { recursive: true })

  console.log('📝 Generating docker-compose configuration...')
  const templatePath = resolve(__dirname, 'docker-compose-test-template.yaml')
  const dockerCompose = readFileSync(templatePath, 'utf-8')
    .replace(/\$\{RUN_ID\}/g, runId)
    .replace(/\$\{POSTGRES_PORT\}/g, postgresPort.toString())
    // The core template uses `ziee-test-postgres-…`; rewrite to
    // `ziee-desktop-test-postgres-…` so desktop + core test runs don't
    // collide on container names.
    .replace(/ziee-test-postgres/g, 'ziee-desktop-test-postgres')
    .replace(/postgres-test-/g, 'postgres-desktop-test-')
    .replace(/name: ziee-test-/, 'name: ziee-desktop-test-')

  const dockerComposePath = resolve(
    configDir,
    `docker-compose-${runId}.yaml`,
  )
  writeFileSync(dockerComposePath, dockerCompose)

  const configData = { runId, port: postgresPort, dockerComposePath }
  writeFileSync(
    resolve(configDir, `postgres-${runId}.json`),
    JSON.stringify(configData, null, 2),
  )

  console.log(`🐘 Starting PostgreSQL container for run ${runId}...`)
  try {
    execSync(`docker compose -f "${dockerComposePath}" up -d`, {
      stdio: 'inherit',
    })
  } catch (err) {
    console.error('❌ Failed to start PostgreSQL container')
    throw err
  }

  console.log('⏳ Waiting for PostgreSQL to be ready...')
  await new Promise(r => setTimeout(r, 3000))

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
    } catch (err) {
      retries--
      if (retries === 0) {
        console.error('❌ Postgres not reachable after 30s:', err)
        await pool.end()
        throw err
      }
      await new Promise(r => setTimeout(r, 1000))
    }
  }
  await pool.end()

  console.log('✅ Desktop test infrastructure ready!')
  console.log(`   - PostgreSQL: port ${postgresPort} (ziee-desktop-test-postgres-${runId})`)
  console.log('   - Vite dev server: http://localhost:1420 (started by webServer)')
  console.log('   - Each test: unique DB + spawned backend on a worker-locked port\n')
}
