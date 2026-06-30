import { FullConfig } from '@playwright/test'
import { execSync } from 'child_process'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'fs'
import { tmpdir } from 'os'
import crypto from 'crypto'
import pg from 'pg'
import dotenv from 'dotenv'
import { cleanupStaleLocks, cleanupStaleConfigFiles, allocatePostgresPort } from './fixtures/port-manager'

const { Pool } = pg
const __dirname = dirname(fileURLToPath(import.meta.url))

export default async function globalSetup(_config: FullConfig) {
  // Load environment variables from .env.test
  dotenv.config({ path: resolve(__dirname, '.env.test') })

  console.log('\n🚀 Starting Playwright E2E Test Infrastructure...\n')

  // Ensure the Playwright browser binary is present. Idempotent + fast when
  // already installed (just verifies the cache); on a fresh machine/CI it
  // downloads chromium so the run doesn't fail every test with
  // "Executable doesn't exist ... npx playwright install". Baked in here so no
  // manual `npx playwright install` step is needed for any e2e invocation.
  try {
    console.log('🌐 Ensuring Playwright chromium is installed...')
    execSync('npx playwright install chromium', { stdio: 'inherit' })
  } catch (e) {
    console.warn('⚠️  playwright install chromium failed (continuing):', e)
  }

  // Clean up stale port locks from previous crashed/killed test runs
  cleanupStaleLocks()

  // Clean up stale config files from previous crashed/killed test runs
  const configDir = resolve(__dirname, '.test-configs')
  cleanupStaleConfigFiles(configDir)

  // Clean up any stale PostgreSQL test containers
  // Only remove containers whose lock files are missing or stale
  console.log('🧹 Cleaning up stale PostgreSQL containers...')
  try {
    const containers = execSync('docker ps -a --filter "name=ziee-tailtest-postgres-" --format "{{.Names}}"', {
      encoding: 'utf-8',
    }).trim()

    if (containers) {
      const containerList = containers.split('\n')
      let removed = 0
      let kept = 0

      for (const container of containerList) {
        // Extract run ID from container name: ziee-test-postgres-{runId}
        const runId = container.replace('ziee-tailtest-postgres-', '')
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

  // 1. Get or generate unique test run ID (config may have already set it)
  const runId = process.env.TEST_RUN_ID || crypto.randomBytes(4).toString('hex')
  console.log(`🆔 Test run ID: ${runId}`)

  // Store runId in environment for teardown and test-context
  process.env.TEST_RUN_ID = runId

  // 2. Allocate PostgreSQL port with file lock
  console.log('🔍 Allocating PostgreSQL port...')
  const postgresPort = await allocatePostgresPort(runId)
  console.log(`✅ Allocated PostgreSQL port: ${postgresPort}\n`)

  // 3. Create .test-configs directory if it doesn't exist (already created above)
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

  // 8. Build the UI ONCE for static `vite preview` serving. The HMR dev server
  //    refuses a SECOND concurrent browser context (multi-context sync specs
  //    open 2-3 contexts), so tests serve a static build instead — which has no
  //    such limit. Built with react+tailwind only (NOT the prod
  //    `removeDataTestPlugin`) so the `data-test-*` selectors the E2E suite
  //    relies on survive. Set E2E_SKIP_BUILD=1 to reuse an existing build.
  const uiRoot = resolve(__dirname, '..')
  const distDir = resolve(uiRoot, 'dist-e2e')
  if (process.env.E2E_SKIP_BUILD === '1' && existsSync(resolve(distDir, 'index.html'))) {
    console.log('🏗️  E2E_SKIP_BUILD=1 — reusing existing dist-e2e build\n')
  } else {
    console.log('🏗️  Building UI for static preview (once per run)...')
    const srcRoot = resolve(uiRoot, 'src')
    const buildCfg = resolve(configDir, 'vite-e2e-build.ts')
    writeFileSync(
      buildCfg,
      `import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  root: ${JSON.stringify(srcRoot)},
  cacheDir: ${JSON.stringify(resolve(uiRoot, 'node_modules/.vite-e2e-build'))},
  resolve: {
    alias: { '@': path.resolve(${JSON.stringify(uiRoot)}, './src') },
    // Force a SINGLE copy of each shared singleton into the bundle. Without
    // this, a prebundled dep can pull a second copy of react/immer/zustand/…
    // whose internal state (React dispatcher, Immer drafts) diverges from the
    // app's copy → the app boot-crashes into AppErrorBoundary at the root
    // ("Cannot read properties of null (reading 'useEffect')", "[Immer]
    // minified error nr: 0", …) and EVERY spec fails. Mirrors the proven
    // dedupe list in src-app/desktop/ui/vite.config.ts.
    dedupe: [
      'react',
      'react-dom',
      'react-router-dom',
      'zustand',
      'antd',
      '@ant-design/icons',
      'i18next',
      'react-i18next',
      'react-icons',
      'react-use',
      'dayjs',
      'immer',
      'tinycolor2',
      'overlayscrollbars',
      'overlayscrollbars-react',
      'streamdown',
      'mermaid',
    ],
  },
  optimizeDeps: { include: ['streamdown', 'streamdown/dist/*.js'] },
  build: { outDir: ${JSON.stringify(distDir)}, emptyOutDir: true },
})
`,
    )
    execSync(`npx vite build --config "${buildCfg}"`, {
      cwd: uiRoot,
      stdio: 'inherit',
    })
    console.log('✅ UI build ready for preview\n')
  }

  console.log('   Test infrastructure:')
  console.log(`   - PostgreSQL: port ${postgresPort} (container: ziee-tailtest-postgres-${runId})`)
  console.log('   - Each worker: 2 dynamic ports (vite preview + backend)')
  console.log('   - Each test: unique database + backend restart')
  console.log('   - Worker 0: 9000+9100, Worker 1: 9001+9101, etc.\n')
}
