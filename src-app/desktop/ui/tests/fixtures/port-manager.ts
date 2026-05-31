/**
 * Port-manager — file-lock-based port allocation for per-test backend
 * spawn. Adapted from `src-app/ui/tests/fixtures/port-manager.ts`.
 *
 * Differences from core:
 *   - Desktop doesn't allocate a per-test Vite port; the single Vite at
 *     1420 (started by Playwright's `webServer` config) is shared by
 *     every worker. We only allocate ONE backend port per test (plus
 *     one Postgres port per test run).
 *   - Lock file names: `backend-<port>.lock` instead of
 *     `ports-<vite>-<backend>.lock`.
 *
 * Locks live in `${tmpdir}/ziee-desktop-test-locks/`. Each lock holds
 * the owning PID + a timestamp heartbeat; a lock with no heartbeat
 * update in HEARTBEAT_STALE_MS is treated as orphaned and reaped.
 *
 * Multi-process safety: file rename on the same filesystem is atomic
 * on POSIX, but we're using write+JSON.parse races. In practice the
 * test orchestrator is one Playwright process spawning N workers in
 * the same Node tree, so PID checks via `process.kill(pid, 0)` are
 * accurate. Concurrent test runs from different shells are rare but
 * supported via the heartbeat reap.
 */

import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'fs'
import { resolve } from 'path'
import { tmpdir } from 'os'
import { execSync } from 'child_process'

const LOCK_DIR = resolve(tmpdir(), 'ziee-desktop-test-locks')
const LOCK_TIMEOUT_MS = 180_000 // 3 min max test duration
const HEARTBEAT_STALE_MS = 10_000 // 10s without heartbeat = orphaned
const CONFIG_STALE_MS = 300_000 // 5 min — wipe leftover config files this old

interface BackendPortLock {
  pid: number // Owning Playwright worker PID
  timestamp: number // Heartbeat ms-since-epoch
  backendPort: number
  backendPid?: number // Spawned cargo PID (filled by test-context)
}

interface PostgresPortLock {
  pid: number
  timestamp: number
  port: number
  runId: string
}

function isProcessAlive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

/** Kill every process listening on the given TCP port. Best-effort. */
function killProcessOnPort(port: number): void {
  try {
    const cmd =
      process.platform === 'win32'
        ? `netstat -ano | findstr :${port}`
        : `lsof -ti :${port}`
    const output = execSync(cmd, { encoding: 'utf8', stdio: 'pipe' }).trim()
    if (!output) return

    if (process.platform === 'win32') {
      const lines = output.split('\n')
      const pids = new Set<string>()
      for (const line of lines) {
        const match = line.trim().match(/\s+(\d+)$/)
        if (match) pids.add(match[1])
      }
      for (const pid of pids) {
        try {
          execSync(`taskkill /F /PID ${pid}`, { stdio: 'ignore' })
        } catch {}
      }
    } else {
      const pids = output.split('\n').filter(Boolean)
      for (const pid of pids) {
        try {
          execSync(`kill -9 ${pid}`, { stdio: 'ignore' })
        } catch {}
      }
    }
    console.log(`🔪 Killed processes on port ${port}`)
  } catch {
    // No process on port — fine.
  }
}

function isBackendLockValid(lock: BackendPortLock): boolean {
  if (Date.now() - lock.timestamp > HEARTBEAT_STALE_MS) {
    console.log(
      `🧹 Backend lock heartbeat stale (last ${new Date(lock.timestamp).toISOString()})`,
    )
    return false
  }
  return true
}

function acquireBackendPortLock(port: number): boolean {
  if (!existsSync(LOCK_DIR)) {
    mkdirSync(LOCK_DIR, { recursive: true })
  }
  const lockFile = resolve(LOCK_DIR, `backend-${port}.lock`)

  if (existsSync(lockFile)) {
    try {
      const existing: BackendPortLock = JSON.parse(
        readFileSync(lockFile, 'utf-8'),
      )
      if (isBackendLockValid(existing)) {
        return false
      }
      console.log(`🧹 Removing stale backend lock for port ${port}`)
      try {
        killProcessOnPort(port)
      } catch {}
      unlinkSync(lockFile)
    } catch {
      console.log(`🧹 Removing corrupted backend lock: ${lockFile}`)
      unlinkSync(lockFile)
    }
  }

  const lock: BackendPortLock = {
    pid: process.pid,
    timestamp: Date.now(),
    backendPort: port,
  }
  writeFileSync(lockFile, JSON.stringify(lock, null, 2))
  return true
}

export function updateBackendLockHeartbeat(
  backendPort: number,
  backendPid?: number,
): void {
  const lockFile = resolve(LOCK_DIR, `backend-${backendPort}.lock`)
  try {
    if (existsSync(lockFile)) {
      const lock: BackendPortLock = JSON.parse(readFileSync(lockFile, 'utf-8'))
      if (lock.pid === process.pid) {
        lock.timestamp = Date.now()
        if (backendPid !== undefined) lock.backendPid = backendPid
        writeFileSync(lockFile, JSON.stringify(lock, null, 2))
      }
    }
  } catch {
    // Best-effort heartbeat
  }
}

export function releaseBackendPortLock(backendPort: number): void {
  const lockFile = resolve(LOCK_DIR, `backend-${backendPort}.lock`)
  try {
    if (existsSync(lockFile)) {
      const lock: BackendPortLock = JSON.parse(readFileSync(lockFile, 'utf-8'))
      if (lock.pid === process.pid) {
        unlinkSync(lockFile)
        console.log(`🔓 Released backend port lock: ${backendPort}`)
      }
    }
  } catch {
    // Best-effort cleanup
  }
}

/**
 * Find an unused backend port + lock it for this worker. Tries up to
 * 100 candidates spread across workers (worker N gets first try at
 * BASE + N, then BASE + N + 8, …) to minimize collisions.
 */
export async function findAvailableBackendPort(
  workerIndex: number,
): Promise<number> {
  const MAX_ATTEMPTS = 100
  const BASE_PORT = 9100

  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    const port = BASE_PORT + workerIndex + attempt * 8
    if (acquireBackendPortLock(port)) {
      console.log(
        `🔒 Locked backend port for worker ${workerIndex}: ${port}`,
      )
      return port
    }
  }
  throw new Error(
    `Could not find available backend port after ${MAX_ATTEMPTS} attempts ` +
      `(worker ${workerIndex}, base ${BASE_PORT})`,
  )
}

export function cleanupStaleLocks(): void {
  if (!existsSync(LOCK_DIR)) return
  console.log('🧹 Cleaning up stale backend port locks...')

  const lockFiles = readdirSync(LOCK_DIR).filter(
    f => f.endsWith('.lock') && f.startsWith('backend-'),
  )
  let removed = 0
  let kept = 0

  for (const lockFile of lockFiles) {
    const lockPath = resolve(LOCK_DIR, lockFile)
    try {
      const lock: BackendPortLock = JSON.parse(readFileSync(lockPath, 'utf-8'))
      if (!isBackendLockValid(lock)) {
        console.log(`   🔪 Killing process on stale port ${lock.backendPort}`)
        try {
          killProcessOnPort(lock.backendPort)
        } catch {}
        unlinkSync(lockPath)
        console.log(`   🗑️  Removed stale lock: ${lockFile}`)
        removed++
      } else {
        console.log(`   ✅ Kept valid lock: ${lockFile} (PID ${lock.pid})`)
        kept++
      }
    } catch {
      unlinkSync(lockPath)
      console.log(`   🗑️  Removed corrupted lock: ${lockFile}`)
      removed++
    }
  }
  console.log(`✅ Lock cleanup: ${removed} removed, ${kept} kept\n`)
}

// ─── Postgres port lock (one per test run, not per test) ────────────

function isPostgresLockValid(lock: PostgresPortLock): boolean {
  const now = Date.now()
  if (now - lock.timestamp > LOCK_TIMEOUT_MS) {
    console.log(`🧹 PostgreSQL lock expired (age ${now - lock.timestamp}ms)`)
    return false
  }
  if (!isProcessAlive(lock.pid)) {
    console.log(`🧹 PostgreSQL lock orphaned (PID ${lock.pid} not running)`)
    return false
  }
  return true
}

function acquirePostgresPortLock(port: number, runId: string): boolean {
  if (!existsSync(LOCK_DIR)) {
    mkdirSync(LOCK_DIR, { recursive: true })
  }
  const lockFile = resolve(LOCK_DIR, `postgres-${port}.lock`)

  if (existsSync(lockFile)) {
    try {
      const existing: PostgresPortLock = JSON.parse(
        readFileSync(lockFile, 'utf-8'),
      )
      if (isPostgresLockValid(existing)) return false
      console.log(`🧹 Removing stale PostgreSQL lock: ${lockFile}`)
      unlinkSync(lockFile)
    } catch {
      console.log(`🧹 Removing corrupted PostgreSQL lock: ${lockFile}`)
      unlinkSync(lockFile)
    }
  }

  const lock: PostgresPortLock = {
    pid: process.pid,
    timestamp: Date.now(),
    port,
    runId,
  }
  writeFileSync(lockFile, JSON.stringify(lock, null, 2))
  return true
}

export async function allocatePostgresPort(runId: string): Promise<number> {
  // Base port 54431: above core's 54331 so two test runs (web + desktop)
  // can coexist without colliding.
  const BASE_PORT = 54431
  const MAX_ATTEMPTS = 100

  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    const port = BASE_PORT + attempt
    if (acquirePostgresPortLock(port, runId)) {
      console.log(
        `🔒 Locked PostgreSQL port ${port} for test run ${runId}`,
      )
      return port
    }
  }
  throw new Error(
    `Could not find available PostgreSQL port after ${MAX_ATTEMPTS} attempts (base ${BASE_PORT})`,
  )
}

export function releasePostgresPortLock(port: number): void {
  const lockFile = resolve(LOCK_DIR, `postgres-${port}.lock`)
  try {
    if (existsSync(lockFile)) {
      const lock: PostgresPortLock = JSON.parse(readFileSync(lockFile, 'utf-8'))
      if (lock.pid === process.pid) {
        unlinkSync(lockFile)
        console.log(`🔓 Released PostgreSQL port lock: ${port}`)
      }
    }
  } catch {
    // Best-effort
  }
}

/**
 * Wipe leftover per-test config files (docker-compose, backend YAML)
 * older than CONFIG_STALE_MS. Run at startup so a crashed previous
 * run doesn't leave gigabytes of orphaned configs on disk.
 */
export function cleanupStaleConfigFiles(configDir: string): void {
  if (!existsSync(configDir)) return
  console.log('🧹 Cleaning up stale config files...')

  const now = Date.now()
  const all = readdirSync(configDir)
  const candidates = all.filter(
    f =>
      (f.startsWith('test-') && f.endsWith('.yaml')) ||
      (f.startsWith('docker-compose-') && f.endsWith('.yaml')) ||
      (f.startsWith('postgres-') && f.endsWith('.json')),
  )
  let removed = 0

  for (const file of candidates) {
    const path = resolve(configDir, file)
    try {
      const age = now - statSync(path).mtimeMs
      if (age > CONFIG_STALE_MS) {
        unlinkSync(path)
        console.log(`   🗑️  Removed stale config: ${file}`)
        removed++
      }
    } catch {
      try {
        unlinkSync(path)
        console.log(`   🗑️  Removed corrupted config: ${file}`)
        removed++
      } catch {}
    }
  }
  console.log(`✅ Config cleanup: ${removed} removed\n`)
}
