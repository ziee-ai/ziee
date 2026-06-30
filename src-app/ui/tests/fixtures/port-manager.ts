import { existsSync, readFileSync, writeFileSync, unlinkSync, mkdirSync, readdirSync, statSync } from 'fs'
import { resolve } from 'path'
import { tmpdir } from 'os'
import { execSync } from 'child_process'
import { createServer } from 'net'

/**
 * True when `port` can actually be bound on 0.0.0.0 right now.
 *
 * The lock file alone is NOT sufficient: a sibling worktree's postgres
 * docker container can keep holding the port (0.0.0.0:port) AFTER the
 * process that launched it exits — at which point its lock is reaped as
 * "orphaned PID" and the port looks free to the lock allocator, but
 * `docker compose up` then fails with "port is already allocated". Mirror
 * the backend harness's portpicker bind-verify: try a real bind. Bind on
 * 0.0.0.0 (not 127.0.0.1) so a docker 0.0.0.0 publish is detected.
 */
function isPortBindable(port: number): Promise<boolean> {
  return new Promise((res) => {
    const srv = createServer()
    srv.once('error', () => res(false))
    srv.once('listening', () => srv.close(() => res(true)))
    srv.listen(port, '0.0.0.0')
  })
}

// Lock dir is env-overridable so concurrent git worktrees can isolate
// their E2E runs from each other. Without this, every worktree shares
// `/tmp/ziee-test-locks` + the same 9000/9100 port base, and one run's
// stale-lock cleanup kills a sibling worktree's just-starting backend
// (observed as a graceful "Shutdown signal received" ~20s into startup).
// Pair with ZIEE_E2E_BASE_VITE_PORT / ZIEE_E2E_BASE_BACKEND_PORT.
const LOCK_DIR = process.env.ZIEE_E2E_LOCK_DIR || resolve(tmpdir(), 'ziee-test-locks')
const LOCK_TIMEOUT_MS = 180000 // 3 minutes - max test duration
// @ts-ignore - Reserved for future use
const _HEARTBEAT_INTERVAL_MS = 5000 // 5 seconds - heartbeat update frequency (reserved for future use)
const HEARTBEAT_STALE_MS = 10000 // 10 seconds - consider stale if no heartbeat
const CONFIG_STALE_MS = 300000 // 5 minutes - clean up config files older than this

interface PortLock {
  pid: number // Main test process PID
  timestamp: number // Last heartbeat timestamp
  vitePort: number
  backendPort: number
  vitePid?: number // Vite process PID
  backendPid?: number // Backend process PID
}

interface PostgresPortLock {
  pid: number
  timestamp: number
  port: number
  runId: string
}

/**
 * Check if a process is still running
 * Uses signal 0 which doesn't kill the process, just checks existence
 * Works on both Linux and Windows
 */
function isProcessAlive(pid: number): boolean {
  try {
    // Send signal 0 to check if process exists without killing it
    process.kill(pid, 0)
    return true
  } catch (e) {
    // ESRCH = no such process
    return false
  }
}

/**
 * Kill a process by PID
 * Works on both Linux and Windows
 * Note: Currently unused in favor of killProcessOnPort for better reliability
 */
// @ts-ignore - Reserved for future use
function _killProcess(pid: number): void {
  try {
    if (process.platform === 'win32') {
      execSync(`taskkill /F /PID ${pid}`, { stdio: 'ignore' })
    } else {
      execSync(`kill -9 ${pid}`, { stdio: 'ignore' })
    }
    console.log(`🔪 Killed process PID ${pid}`)
  } catch (e) {
    // Process might already be dead
  }
}

/**
 * Kill all processes using a specific port
 * More reliable than killing by PID for parent/child process trees
 */
function killProcessOnPort(port: number): void {
  try {
    const cmd = process.platform === 'win32'
      ? `netstat -ano | findstr :${port}`
      : `lsof -ti :${port}`

    const output = execSync(cmd, { encoding: 'utf8', stdio: 'pipe' }).trim()

    if (!output) return

    if (process.platform === 'win32') {
      // Windows: extract PID from netstat output
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
      // Unix: lsof returns PIDs directly
      const pids = output.split('\n').filter(Boolean)
      for (const pid of pids) {
        try {
          execSync(`kill -9 ${pid}`, { stdio: 'ignore' })
        } catch {}
      }
    }

    console.log(`🔪 Killed processes on port ${port}`)
  } catch (error) {
    // No process on port or command failed - that's ok
  }
}

/**
 * Validate if a lock is still valid.
 *
 * A lock is valid if either:
 *   1. The heartbeat timestamp is recent (within HEARTBEAT_STALE_MS), OR
 *   2. The recorded process PIDs are still alive (signal-0 probe).
 *
 * The PID liveness check (#2) is critical: under load (cargo cold-start
 * compilation, vite optimize-deps reload, the test node event loop
 * pegged waiting on a long fetch handler) the heartbeat interval can
 * drift past the 10 s stale threshold even while the test is healthy
 * mid-execution. If we treated that as stale and killed the processes,
 * we'd take down ANOTHER worker's running backend and vite — that's
 * exactly the symptom users saw as "ERR_CONNECTION_REFUSED" /
 * "TypeError: Failed to fetch" cascades during parallel runs.
 *
 * Only treat the lock as stale when BOTH the heartbeat is gone AND
 * the registered processes are gone.
 */
function isLockValid(lock: PortLock): boolean {
  const now = Date.now()

  if (now - lock.timestamp <= HEARTBEAT_STALE_MS) {
    return true
  }

  // Heartbeat is stale — but check if the registered processes are
  // actually still running. A live process means the worker is just
  // blocked (event loop), not crashed.
  const mainAlive = isProcessAlive(lock.pid)
  const viteAlive = lock.vitePid ? isProcessAlive(lock.vitePid) : false
  const backendAlive = lock.backendPid ? isProcessAlive(lock.backendPid) : false

  if (mainAlive || viteAlive || backendAlive) {
    // Workers still alive — heartbeat just hasn't ticked yet. Treat as
    // valid; the worker will catch up.
    return true
  }

  console.log(
    `🧹 Lock heartbeat stale AND all registered PIDs are dead ` +
      `(last update: ${new Date(lock.timestamp).toISOString()}, ` +
      `age: ${now - lock.timestamp}ms, main pid=${lock.pid}, ` +
      `vite pid=${lock.vitePid ?? 'unset'}, backend pid=${lock.backendPid ?? 'unset'})`,
  )
  return false
}

/**
 * Try to acquire a lock for a specific port pair
 * Returns true if lock was acquired, false if ports are in use
 * If lock is stale, kills processes on those ports before acquiring
 */
function acquirePortLock(vitePort: number, backendPort: number): boolean {
  if (!existsSync(LOCK_DIR)) {
    mkdirSync(LOCK_DIR, { recursive: true })
  }

  const lockFile = resolve(LOCK_DIR, `ports-${vitePort}-${backendPort}.lock`)

  // Check existing lock
  if (existsSync(lockFile)) {
    try {
      const existingLock: PortLock = JSON.parse(readFileSync(lockFile, 'utf-8'))

      if (isLockValid(existingLock)) {
        // Lock is valid, ports are in use
        return false
      } else {
        // Lock is stale, kill processes on these ports
        console.log(`🧹 Removing stale lock: ${lockFile}`)
        console.log(`🔪 Killing processes on ports ${vitePort} and ${backendPort}`)

        // Kill by port instead of PID - more reliable for parent/child process trees
        try {
          killProcessOnPort(vitePort)
          killProcessOnPort(backendPort)
        } catch (e) {
          // Best effort
        }

        unlinkSync(lockFile)
      }
    } catch (e) {
      // Corrupted lock file, remove it
      console.log(`🧹 Removing corrupted lock: ${lockFile}`)
      unlinkSync(lockFile)
    }
  }

  // Acquire lock
  const lock: PortLock = {
    pid: process.pid,
    timestamp: Date.now(),
    vitePort,
    backendPort,
  }

  writeFileSync(lockFile, JSON.stringify(lock, null, 2))
  return true
}

/**
 * Update the heartbeat for a port lock
 * Updates timestamp and process PIDs to keep lock alive
 */
export function updatePortLockHeartbeat(
  vitePort: number,
  backendPort: number,
  vitePid?: number,
  backendPid?: number
): void {
  const lockFile = resolve(LOCK_DIR, `ports-${vitePort}-${backendPort}.lock`)

  try {
    if (existsSync(lockFile)) {
      const lock: PortLock = JSON.parse(readFileSync(lockFile, 'utf-8'))

      // Only update if we own the lock
      if (lock.pid === process.pid) {
        lock.timestamp = Date.now()
        if (vitePid !== undefined) {
          lock.vitePid = vitePid
        }
        if (backendPid !== undefined) {
          lock.backendPid = backendPid
        }

        writeFileSync(lockFile, JSON.stringify(lock, null, 2))
      }
    }
  } catch (e) {
    // Best effort heartbeat update
  }
}

/**
 * Release a port lock
 * Only releases if we own the lock (matching PID)
 */
export function releasePortLock(vitePort: number, backendPort: number): void {
  const lockFile = resolve(LOCK_DIR, `ports-${vitePort}-${backendPort}.lock`)

  try {
    if (existsSync(lockFile)) {
      const lock: PortLock = JSON.parse(readFileSync(lockFile, 'utf-8'))

      // Only release if we own the lock
      if (lock.pid === process.pid) {
        unlinkSync(lockFile)
        console.log(`🔓 Released port lock: ${vitePort}/${backendPort}`)
      }
    }
  } catch (e) {
    // Best effort cleanup
  }
}

/**
 * Find and lock an available port pair for this test worker
 * Tries multiple port ranges to find free ports
 * Automatically cleans up stale locks from crashed processes
 */
export async function findAvailablePorts(
  workerIndex: number
): Promise<{ vite: number; backend: number }> {
  // Try up to 100 port pairs
  const MAX_ATTEMPTS = 100
  // Env-overridable port base for cross-worktree isolation (see LOCK_DIR).
  const BASE_VITE_PORT = parseInt(process.env.ZIEE_E2E_BASE_VITE_PORT || '9000', 10)
  const BASE_BACKEND_PORT = parseInt(process.env.ZIEE_E2E_BASE_BACKEND_PORT || '9100', 10)

  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    // Spread workers across port range to reduce collisions
    // First try: worker 0 = 9000, worker 1 = 9001, etc.
    // Second try: worker 0 = 9008, worker 1 = 9009, etc.
    const offset = workerIndex + attempt * 8
    const vitePort = BASE_VITE_PORT + offset
    const backendPort = BASE_BACKEND_PORT + offset

    if (acquirePortLock(vitePort, backendPort)) {
      console.log(`🔒 Locked ports for worker ${workerIndex}: vite ${vitePort}, backend ${backendPort}`)
      return { vite: vitePort, backend: backendPort }
    }
  }

  throw new Error(
    `Could not find available ports after ${MAX_ATTEMPTS} attempts for worker ${workerIndex}`
  )
}

/**
 * Clean up all stale port locks from crashed/killed test processes
 * This should be called in global-setup to ensure a clean state before tests start
 * Kills processes on stale ports before removing locks
 */
export function cleanupStaleLocks(): void {
  if (!existsSync(LOCK_DIR)) {
    return
  }

  console.log('🧹 Cleaning up stale port locks...')

  const lockFiles = readdirSync(LOCK_DIR).filter(file => file.endsWith('.lock') && file.startsWith('ports-'))
  let removed = 0
  let kept = 0

  for (const lockFile of lockFiles) {
    const lockPath = resolve(LOCK_DIR, lockFile)

    try {
      const lock: PortLock = JSON.parse(readFileSync(lockPath, 'utf-8'))

      if (!isLockValid(lock)) {
        // Kill processes on these ports before removing lock
        console.log(`   🔪 Killing processes on ports ${lock.vitePort}/${lock.backendPort}`)
        try {
          killProcessOnPort(lock.vitePort)
          killProcessOnPort(lock.backendPort)
        } catch (e) {
          // Best effort
        }

        unlinkSync(lockPath)
        console.log(`   🗑️  Removed stale lock: ${lockFile} (PID ${lock.pid})`)
        removed++
      } else {
        console.log(`   ✅ Kept valid lock: ${lockFile} (PID ${lock.pid})`)
        kept++
      }
    } catch (e) {
      // Corrupted lock file, remove it
      unlinkSync(lockPath)
      console.log(`   🗑️  Removed corrupted lock: ${lockFile}`)
      removed++
    }
  }

  console.log(`✅ Cleanup complete: ${removed} removed, ${kept} kept\n`)
}

/**
 * Validate PostgreSQL port lock
 */
function isPostgresLockValid(lock: PostgresPortLock): boolean {
  const now = Date.now()

  // Check if lock is expired (timeout fallback)
  if (now - lock.timestamp > LOCK_TIMEOUT_MS) {
    console.log(`🧹 PostgreSQL lock expired (timestamp: ${lock.timestamp}, now: ${now})`)
    return false
  }

  // Check if process is still alive
  if (!isProcessAlive(lock.pid)) {
    console.log(`🧹 PostgreSQL lock orphaned (PID ${lock.pid} not running)`)
    return false
  }

  return true
}

/**
 * Try to acquire a lock for a specific PostgreSQL port
 * Returns true if lock was acquired, false if port is in use
 */
function acquirePostgresPortLock(port: number, runId: string): boolean {
  if (!existsSync(LOCK_DIR)) {
    mkdirSync(LOCK_DIR, { recursive: true })
  }

  const lockFile = resolve(LOCK_DIR, `postgres-${port}.lock`)

  // Check existing lock
  if (existsSync(lockFile)) {
    try {
      const existingLock: PostgresPortLock = JSON.parse(readFileSync(lockFile, 'utf-8'))

      if (isPostgresLockValid(existingLock)) {
        // Lock is valid, port is in use
        return false
      } else {
        // Lock is stale, remove it
        console.log(`🧹 Removing stale PostgreSQL lock: ${lockFile}`)
        unlinkSync(lockFile)
      }
    } catch (e) {
      // Corrupted lock file, remove it
      console.log(`🧹 Removing corrupted PostgreSQL lock: ${lockFile}`)
      unlinkSync(lockFile)
    }
  }

  // Acquire lock
  const lock: PostgresPortLock = {
    pid: process.pid,
    timestamp: Date.now(),
    port,
    runId,
  }

  writeFileSync(lockFile, JSON.stringify(lock, null, 2))
  return true
}

/**
 * Allocate a PostgreSQL port for this test run
 * Tries multiple ports starting from base port 54331
 * Returns the allocated port number
 */
export async function allocatePostgresPort(runId: string): Promise<number> {
  const BASE_PORT = 54331
  const MAX_ATTEMPTS = 100

  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    const port = BASE_PORT + attempt

    if (!acquirePostgresPortLock(port, runId)) {
      continue
    }

    // Lock acquired — but ALSO verify the port is free at the OS level. A
    // sibling worktree's leftover docker container can still hold the port
    // even though its lock was reaped (its launching PID is gone), so a
    // lock-free port is not necessarily bind-free. Without this check the
    // allocator hands out an already-bound port and globalSetup's
    // `docker compose up` dies with "port is already allocated", failing
    // the ENTIRE run.
    if (await isPortBindable(port)) {
      console.log(`🔒 Locked PostgreSQL port ${port} for test run ${runId}`)
      return port
    }

    // Bound by something else (almost always a sibling's orphaned
    // container) — release our just-taken lock and try the next port.
    console.log(
      `⚠️  PostgreSQL port ${port} is lock-free but already bound at the OS level; skipping`,
    )
    releasePostgresPortLock(port)
  }

  throw new Error(
    `Could not find available PostgreSQL port after ${MAX_ATTEMPTS} attempts (base: ${BASE_PORT})`
  )
}

/**
 * Release a PostgreSQL port lock
 * Only releases if we own the lock (matching PID)
 */
export function releasePostgresPortLock(port: number): void {
  const lockFile = resolve(LOCK_DIR, `postgres-${port}.lock`)

  try {
    if (existsSync(lockFile)) {
      const lock: PostgresPortLock = JSON.parse(readFileSync(lockFile, 'utf-8'))

      // Only release if we own the lock
      if (lock.pid === process.pid) {
        unlinkSync(lockFile)
        console.log(`🔓 Released PostgreSQL port lock: ${port}`)
      }
    }
  } catch (e) {
    // Best effort cleanup
  }
}

/**
 * Clean up stale test config files from crashed/killed test processes
 * Removes vite-*.ts and test-*.yaml files older than CONFIG_STALE_MS
 * This should be called in global-setup to ensure a clean state before tests start
 */
export function cleanupStaleConfigFiles(configDir: string): void {
  if (!existsSync(configDir)) {
    return
  }

  console.log('🧹 Cleaning up stale config files...')

  const now = Date.now()
  const allFiles = readdirSync(configDir)
  const files = allFiles.filter(
    file => (file.startsWith('vite-') && file.endsWith('.ts')) ||
            (file.startsWith('test-') && file.endsWith('.yaml')) ||
            (file.startsWith('docker-compose-') && file.endsWith('.yaml')) ||
            (file.startsWith('postgres-') && file.endsWith('.json'))
  )

  console.log(`   Found ${files.length} test config files to check (${allFiles.length} total files)`)

  let removed = 0

  for (const file of files) {
    const filePath = resolve(configDir, file)

    try {
      const stats = statSync(filePath)
      const age = now - stats.mtimeMs

      // Remove files older than 5 minutes
      if (age > CONFIG_STALE_MS) {
        unlinkSync(filePath)
        console.log(`   🗑️  Removed stale config: ${file} (age: ${Math.round(age / 1000)}s)`)
        removed++
      }
    } catch (e) {
      // Error reading file, try to remove it anyway
      try {
        unlinkSync(filePath)
        console.log(`   🗑️  Removed corrupted config: ${file}`)
        removed++
      } catch {}
    }
  }

  if (removed > 0) {
    console.log(`✅ Config cleanup complete: ${removed} removed\n`)
  } else {
    console.log('✅ No stale config files found\n')
  }
}
