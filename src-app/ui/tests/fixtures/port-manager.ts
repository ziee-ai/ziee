import { existsSync, readFileSync, writeFileSync, unlinkSync, mkdirSync, readdirSync, statSync } from 'fs'
import { resolve } from 'path'
import { tmpdir } from 'os'
import { execSync } from 'child_process'

const LOCK_DIR = resolve(tmpdir(), 'ziee-test-locks')
const LOCK_TIMEOUT_MS = 180000 // 3 minutes - max test duration
const HEARTBEAT_INTERVAL_MS = 5000 // 5 seconds - heartbeat update frequency
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
 */
function killProcess(pid: number): void {
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
 * Validate if a lock is still valid based on heartbeat
 * A lock is valid if the heartbeat timestamp is recent (within HEARTBEAT_STALE_MS)
 */
function isLockValid(lock: PortLock): boolean {
  const now = Date.now()

  // Check if heartbeat is stale (no update for >10 seconds)
  if (now - lock.timestamp > HEARTBEAT_STALE_MS) {
    console.log(`🧹 Lock heartbeat stale (last update: ${new Date(lock.timestamp).toISOString()}, age: ${now - lock.timestamp}ms)`)
    return false
  }

  return true
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
  const BASE_VITE_PORT = 9000
  const BASE_BACKEND_PORT = 9100

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

    if (acquirePostgresPortLock(port, runId)) {
      console.log(`🔒 Locked PostgreSQL port ${port} for test run ${runId}`)
      return port
    }
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
