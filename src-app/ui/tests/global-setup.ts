import { FullConfig } from '@playwright/test'
import { execSync } from 'child_process'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from 'fs'
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

  // Per-session container namespace. Concurrent e2e sessions (separate git
  // worktrees) share ONE docker daemon, so their containers must live in
  // disjoint name-spaces or a starting session reaps a sibling's live one.
  // Each session is handed a distinct ZIEE_E2E_BASE_PG_PORT (see port-manager),
  // so derive the namespace from it: container names become
  // `ziee-tailtest-postgres-pg<base>-<rand>` and the cleanup filter below is
  // scoped to THIS session's prefix — a session can only ever see (and thus
  // reap) its OWN containers. The shared-lock liveness check is the second
  // belt: even within one session it keeps the live current run and only reaps
  // this session's crashed leftovers.
  const sessionNs = `pg${process.env.ZIEE_E2E_BASE_PG_PORT || '54331'}`

  // Clean up any stale PostgreSQL test containers.
  //
  // CROSS-SESSION SAFETY: multiple concurrent e2e sessions (separate git
  // worktrees) share ONE docker daemon, so `docker ps` lists EVERY session's
  // containers. Liveness must therefore be judged from the SHARED lock dir
  // (tmpdir/ziee-test-locks, keyed by pid+runId), NOT the per-worktree
  // `.test-configs/` dir — a sibling session's config lives in ITS worktree and
  // is invisible here, so keying off it wrongly classifies a sibling's LIVE
  // container as stale and `docker rm -f`s it mid-run (→ ECONNREFUSED storms in
  // the victim run). The shared postgres-*.lock files carry {pid, runId}, which
  // is all we need to map a container back to a live owning process.
  console.log('🧹 Cleaning up stale PostgreSQL containers...')
  try {
    const containers = execSync(
      `docker ps -a --filter "name=ziee-tailtest-postgres-${sessionNs}-" --format "{{.Names}}"`,
      { encoding: 'utf-8' },
    ).trim()

    if (containers) {
      const containerList = containers.split('\n')
      let removed = 0
      let kept = 0

      // Build runId → live-pid map from the SHARED lock dir (same location
      // port-manager writes to). Any lock whose owning PID is still alive marks
      // that runId's container as in-use by SOME session on this box.
      const lockDir = process.env.ZIEE_E2E_LOCK_DIR || resolve(tmpdir(), 'ziee-test-locks')
      const liveRunIds = new Set<string>()
      if (existsSync(lockDir)) {
        for (const f of readdirSync(lockDir)) {
          if (!f.startsWith('postgres-') || !f.endsWith('.lock')) continue
          try {
            const lock = JSON.parse(readFileSync(resolve(lockDir, f), 'utf-8'))
            if (!lock.runId) continue
            try {
              process.kill(lock.pid, 0) // Signal 0 just checks the process exists
              liveRunIds.add(lock.runId)
            } catch {
              // Owning process is gone — lock is stale, container is reapable.
            }
          } catch {
            // Corrupted lock file — ignore.
          }
        }
      }

      for (const container of containerList) {
        // Extract run ID from container name: ziee-tailtest-postgres-{runId}
        const runId = container.replace('ziee-tailtest-postgres-', '')

        if (liveRunIds.has(runId)) {
          console.log(`   ✅ Kept active container: ${container} (live lock)`)
          kept++
          continue
        }

        // No live lock owns this runId — safe to reap.
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
  const runId = process.env.TEST_RUN_ID || `${sessionNs}-${crypto.randomBytes(4).toString('hex')}`
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
// Smart-loading needs the module manifest (\`virtual:ziee-module-manifest\`,
// imported by src/modules/loader.ts) — without this plugin the build fails to
// resolve that virtual id. preloadGraphPlugin emits the idle-prefetch graph so
// the prod-mode e2e build matches prod (and doesn't 404 on the graph fetch).
import { moduleManifestPlugin } from ${JSON.stringify(resolve(uiRoot, 'plugins/vite-plugin-module-manifest.js'))}
import { preloadGraphPlugin } from ${JSON.stringify(resolve(uiRoot, 'plugins/vite-plugin-preload-graph.js'))}

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    moduleManifestPlugin({ srcDir: ${JSON.stringify(srcRoot)} }),
    preloadGraphPlugin(),
  ],
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

  // 8b. Warm the server binary ONCE so no per-test `cargo run` pays compilation.
  // Each test spawns `cargo run --bin ziee` behind a 120s readiness budget; on
  // the FIRST spawn of a run (or after a merge/rebase that touched server code)
  // that cargo invocation compiles the whole crate, which under concurrent-test
  // load blows the 120s budget → the "Backend server failed to start on port
  // 91xx" flake that bites every session. Building here serializes the compile
  // OUT of the per-test budget so every spawn is a warm, fast start. build.rs
  // provisions the per-worktree build DB, so no DATABASE_URL is needed. Opt out
  // with E2E_SKIP_SERVER_WARMUP=1. Non-fatal: a failed warmup just falls back to
  // the old per-test (re)build within its own budget.
  if (process.env.E2E_SKIP_SERVER_WARMUP !== '1') {
    console.log('🏗️  Warming the server binary (cargo build --bin ziee)...')
    const serverRoot = resolve(uiRoot, '../server')
    const cargoBin =
      process.platform === 'win32'
        ? `${process.env.USERPROFILE}\\.cargo\\bin\\cargo`
        : `${process.env.HOME}/.cargo/bin/cargo`
    try {
      execSync(`"${cargoBin}" build --bin ziee`, {
        cwd: serverRoot,
        stdio: 'inherit',
      })
      console.log('✅ Server binary warm\n')
    } catch (e) {
      console.warn('⚠️  Server warmup build failed (continuing; per-test cargo run will build):', e)
    }
  }

  console.log('   Test infrastructure:')
  console.log(`   - PostgreSQL: port ${postgresPort} (container: ziee-tailtest-postgres-${runId})`)
  console.log('   - Each worker: 2 dynamic ports (vite preview + backend)')
  console.log('   - Each test: unique database + backend restart')
  console.log('   - Worker 0: 9000+9100, Worker 1: 9001+9101, etc.\n')
}
