/**
 * Per-test backend fixture for desktop Playwright tests.
 *
 * Each test that imports `{ test, expect }` from this file gets:
 *   - A fresh Postgres database (created in the shared container
 *     started by global-setup.ts).
 *   - A spawned `cargo run --bin ziee` server process on a
 *     worker-locked port.
 *   - A bootstrapped admin user (POST /api/app/setup/admin).
 *   - A real login session (POST /api/auth/login) → real JWT tokens.
 *
 * The fixture exposes `testInfra` with the backend port + tokens.
 * Tests pass these to `installTauriMock(page, { backendPort,
 * tokens })`, and the desktop UI's `getBaseUrl()` →
 * `invoke('get_server_port')` → real port; `invoke('auto_login')` →
 * real tokens; `fetch('/api/...')` → real backend. Full stack.
 *
 * Cleanup (after each test): kill the cargo process, drop the
 * database, release the port lock.
 */

import { test as base } from '@playwright/test'
import { spawn, ChildProcess, execSync } from 'child_process'
import {
  existsSync,
  mkdirSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} from 'fs'
import { dirname, resolve } from 'path'
import { fileURLToPath } from 'url'
import crypto from 'crypto'
import pg from 'pg'
import {
  findAvailableBackendPort,
  releaseBackendPortLock,
  updateBackendLockHeartbeat,
} from './port-manager'

const { Pool } = pg
const __dirname = dirname(fileURLToPath(import.meta.url))

export interface AuthTokens {
  user: {
    id: string
    username: string
    email: string
    email_verified: boolean
    is_active: boolean
    is_admin: boolean
    permissions: string[]
    completed_onboarding_ids: string[]
    completed_onboarding_step_ids: string[]
    created_at: string
    updated_at: string
  }
  access_token: string
  refresh_token: string
  expires_in: number
}

export interface TestInfra {
  databaseName: string
  backendPort: number
  backendURL: string
  tokens: AuthTokens
}

interface InternalTestInfra extends TestInfra {
  serverProcess: ChildProcess
  heartbeatInterval: NodeJS.Timeout
  postgresPort: number
}

interface TestFixtures {
  testInfra: TestInfra
}

const BACKEND_STARTUP_TIMEOUT_MS = 90_000 // cold cargo build can be slow
const READINESS_POLL_INTERVAL_MS = 500
const HEARTBEAT_INTERVAL_MS = 5_000

function killProcessOnPort(port: number): void {
  try {
    const cmd =
      process.platform === 'win32'
        ? `netstat -ano | findstr :${port}`
        : `lsof -ti :${port}`
    const output = execSync(cmd, { encoding: 'utf8', stdio: 'pipe' }).trim()
    if (!output) return

    if (process.platform === 'win32') {
      const pids = new Set<string>()
      for (const line of output.split('\n')) {
        const match = line.trim().match(/\s+(\d+)$/)
        if (match) pids.add(match[1])
      }
      for (const pid of pids) {
        try {
          execSync(`taskkill /F /PID ${pid}`, { stdio: 'ignore' })
        } catch {}
      }
    } else {
      for (const pid of output.split('\n').filter(Boolean)) {
        try {
          execSync(`kill -9 ${pid}`, { stdio: 'ignore' })
        } catch {}
      }
    }
  } catch {
    // No listener on port — fine.
  }
}

async function waitForBackendReady(
  backendURL: string,
  deadline: number,
): Promise<void> {
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${backendURL}/api/health`)
      if (res.ok) return
    } catch {
      // Connection refused — backend not yet listening.
    }
    await new Promise(r => setTimeout(r, READINESS_POLL_INTERVAL_MS))
  }
  throw new Error(
    `Backend at ${backendURL} did not become ready within ` +
      `${BACKEND_STARTUP_TIMEOUT_MS}ms`,
  )
}

// Admin bootstrap + tokens are obtained inline in the testInfra
// fixture below (POST /api/app/setup/admin returns AuthResponse —
// user + flattened TokenPair — directly, so no separate /api/auth/login
// round-trip is needed). Request shape per
// `server/src/modules/app/types.rs::SetupAdminRequest`:
//   { username, email, password, display_name? }
// Response shape per `server/src/modules/auth/types.rs::AuthResponse`:
//   { user, access_token, refresh_token, token_type, expires_in }

export const test = base.extend<TestFixtures>({
  testInfra: async ({}, use, testInfo) => {
    const testId = crypto.randomBytes(4).toString('hex')
    const databaseName = `ziee_desktop_test_${testId}`
    const workerIndex = testInfo.workerIndex

    // 1. Read shared Postgres port from global-setup's JSON.
    const runId = process.env.TEST_RUN_ID
    if (!runId) {
      throw new Error('TEST_RUN_ID not set — global-setup may have failed')
    }
    const configDir = resolve(__dirname, '../.test-configs')
    const postgresConfig = JSON.parse(
      readFileSync(resolve(configDir, `postgres-${runId}.json`), 'utf-8'),
    )
    const postgresPort: number = postgresConfig.port

    // 2. Allocate + lock a backend port for this worker.
    const backendPort = await findAvailableBackendPort(workerIndex)
    const backendURL = `http://127.0.0.1:${backendPort}`

    console.log(
      `\n🔧 [${testInfo.title}] db=${databaseName} backend=${backendURL}`,
    )

    // 3. Make sure nothing's lingering on the port (paranoid cleanup).
    console.log(`   [step 3] killProcessOnPort(${backendPort})`)
    killProcessOnPort(backendPort)

    // 4. Create the per-test database.
    console.log(`   [step 4] connecting to admin pool on PG port ${postgresPort}`)
    const adminPool = new Pool({
      host: 'localhost',
      port: postgresPort,
      user: 'postgres',
      password: 'password',
      database: 'postgres',
    })
    try {
      console.log(`   [step 4a] CREATE DATABASE ${databaseName}`)
      await adminPool.query(`CREATE DATABASE ${databaseName}`)
      console.log(`   [step 4b] database created`)
    } finally {
      await adminPool.end()
    }

    // 5. Write the per-test backend YAML config.
    if (!existsSync(configDir)) mkdirSync(configDir, { recursive: true })
    const configPath = resolve(configDir, `test-${testId}.yaml`)
    const yaml = `postgresql:
  use_embedded: false
  external:
    host: "localhost"
    port: ${postgresPort}
    username: "postgres"
    password: "password"
    database: "${databaseName}"
  pool:
    max_connections: 5
    min_connections: 1
    acquire_timeout_secs: 3
    idle_timeout_secs: 10
    max_lifetime_secs: 60

server:
  host: "127.0.0.1"
  port: ${backendPort}
  api_prefix: "/api"
  rate_limit:
    per_second: 10000
    burst_size: 10000
  cors:
    allow_origins:
      - "http://localhost:1420"
      - "http://127.0.0.1:${backendPort}"
    allow_methods:
      - "GET"
      - "POST"
      - "PUT"
      - "DELETE"
      - "OPTIONS"
    allow_headers:
      - "Content-Type"
      - "Authorization"

logging:
  level: "info"
  format: "json"

jwt:
  secret: "desktop-test-jwt-secret-min-32-chars-${testId}"
  issuer: "ziee-desktop-test"
  audience: "ziee-desktop-test-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: 30
`
    writeFileSync(configPath, yaml)

    // 6. Spawn `cargo run --bin ziee`. Workspace root is src-app/;
    //    server crate is src-app/server. From this fixture file
    //    (src-app/desktop/ui/tests/fixtures), the server crate is
    //    `../../../../server`.
    const cargoBin =
      process.platform === 'win32'
        ? `${process.env.USERPROFILE}\\.cargo\\bin\\cargo`
        : `${process.env.HOME}/.cargo/bin/cargo`

    const serverCrateDir = resolve(__dirname, '../../../../server')
    console.log(`   [step 6] spawning cargo from ${serverCrateDir}`)

    const serverProcess = spawn(
      cargoBin,
      ['run', '--bin', 'ziee', '--', '--config-file', configPath],
      {
        cwd: serverCrateDir,
        stdio: ['ignore', 'pipe', 'pipe'],
        detached: false,
        env: {
          ...process.env,
          PATH:
            process.platform === 'win32'
              ? `${process.env.USERPROFILE}\\.cargo\\bin;${process.env.PATH}`
              : `${process.env.HOME}/.cargo/bin:${process.env.PATH}`,
        },
      },
    )
    console.log(`   [step 6a] cargo spawned pid=${serverProcess.pid}`)

    serverProcess.on('error', err => {
      console.error(`[backend ${backendPort}] spawn error:`, err)
    })
    serverProcess.on('exit', (code, signal) => {
      console.error(
        `[backend ${backendPort}] exited code=${code} signal=${signal}`,
      )
    })
    serverProcess.stdout?.on('data', data => {
      // Log everything during debug — silence the "info" noise later.
      console.log(`[backend ${backendPort} stdout] ${data.toString().trim()}`)
    })
    serverProcess.stderr?.on('data', data => {
      console.log(`[backend ${backendPort} stderr] ${data.toString().trim()}`)
    })

    // Update lock heartbeat periodically so the port-manager doesn't
    // reap our lock mid-test.
    if (serverProcess.pid !== undefined) {
      updateBackendLockHeartbeat(backendPort, serverProcess.pid)
    }
    const heartbeatInterval = setInterval(() => {
      updateBackendLockHeartbeat(backendPort, serverProcess.pid)
    }, HEARTBEAT_INTERVAL_MS)

    // 7. Wait for the backend to start serving HTTP. Cold cargo
    //    build (first test of the run) can take 60-90s.
    console.log(`   [step 7] polling ${backendURL}/api/health`)
    const deadline = Date.now() + BACKEND_STARTUP_TIMEOUT_MS
    try {
      await waitForBackendReady(backendURL, deadline)
      console.log(`   [step 7a] backend ready!`)
    } catch (err) {
      clearInterval(heartbeatInterval)
      serverProcess.kill('SIGKILL')
      throw err
    }

    // 8. Bootstrap an admin + log in. Single-admin desktop expects
    //    `admin` to exist; we mint a per-test admin with predictable
    //    creds so each test gets fresh state.
    const adminUsername = 'admin'
    const adminPassword = `desktop-test-${testId}`
    // Use a proper TLD — the server's email validator rejects
    // `@localhost` as invalid format.
    const adminEmail = `admin-${testId}@example.com`

    try {
      // DEBUG: try POST with explicit timeout + AbortController. The
      // worker was dying silently on the bare fetch; we want a real
      // error if it stalls instead of SIGKILL.
      console.log(`   [step 8] POST ${backendURL}/api/app/setup/admin`)
      const ctrl = new AbortController()
      const killer = setTimeout(() => {
        console.error(`   [step 8 TIMEOUT] aborting after 15s`)
        ctrl.abort()
      }, 15_000)
      let tokens: AuthTokens
      try {
        const res = await fetch(`${backendURL}/api/app/setup/admin`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            username: adminUsername,
            password: adminPassword,
            email: adminEmail,
            display_name: 'Test Admin',
          }),
          signal: ctrl.signal,
        })
        console.log(`   [step 8a] response status: ${res.status}`)
        const body = await res.text()
        console.log(`   [step 8b] response body (first 200): ${body.slice(0, 200)}`)
        if (!res.ok) {
          throw new Error(
            `Bootstrap admin failed ${res.status}: ${body.slice(0, 200)}`,
          )
        }
        const json = JSON.parse(body) as AuthTokens & { token_type?: string }
        tokens = {
          user: json.user,
          access_token: json.access_token,
          refresh_token: json.refresh_token,
          expires_in: json.expires_in,
        }
        console.log(`   [step 8c] admin bootstrapped: ${tokens.user.username}`)
      } finally {
        clearTimeout(killer)
      }

      console.log(
        `✅ [${testInfo.title}] backend ready, admin bootstrapped`,
      )

      const infra: InternalTestInfra = {
        databaseName,
        backendPort,
        backendURL,
        tokens,
        serverProcess,
        heartbeatInterval,
        postgresPort,
      }

      await use({
        databaseName: infra.databaseName,
        backendPort: infra.backendPort,
        backendURL: infra.backendURL,
        tokens: infra.tokens,
      })

      // ── Teardown ────────────────────────────────────────────
      clearInterval(infra.heartbeatInterval)

      // Kill the spawned backend.
      if (infra.serverProcess.pid !== undefined) {
        try {
          infra.serverProcess.kill('SIGTERM')
          // Give it a beat to flush; force-kill if needed.
          await new Promise(r => setTimeout(r, 500))
          if (!infra.serverProcess.killed) {
            infra.serverProcess.kill('SIGKILL')
          }
        } catch {}
      }
      killProcessOnPort(infra.backendPort)

      // Drop the per-test database.
      const dropPool = new Pool({
        host: 'localhost',
        port: infra.postgresPort,
        user: 'postgres',
        password: 'password',
        database: 'postgres',
      })
      try {
        // Disconnect any straggler clients before DROP.
        await dropPool.query(
          `SELECT pg_terminate_backend(pid) FROM pg_stat_activity ` +
            `WHERE datname = $1 AND pid <> pg_backend_pid()`,
          [infra.databaseName],
        )
        await dropPool.query(`DROP DATABASE IF EXISTS ${infra.databaseName}`)
      } catch (err) {
        console.warn(
          `⚠️  Failed to drop ${infra.databaseName}: ${(err as Error).message}`,
        )
      } finally {
        await dropPool.end()
      }

      // Release the port lock + delete config file.
      releaseBackendPortLock(infra.backendPort)
      try {
        unlinkSync(configPath)
      } catch {}
    } catch (err) {
      // Teardown on setup failure.
      clearInterval(heartbeatInterval)
      serverProcess.kill('SIGKILL')
      killProcessOnPort(backendPort)
      releaseBackendPortLock(backendPort)
      try {
        unlinkSync(configPath)
      } catch {}
      throw err
    }
  },
})

export { expect } from '@playwright/test'
