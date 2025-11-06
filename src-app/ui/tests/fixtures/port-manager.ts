import { existsSync, readFileSync, writeFileSync, unlinkSync, mkdirSync, readdirSync } from 'fs'
import { resolve } from 'path'
import { tmpdir } from 'os'

const LOCK_DIR = resolve(tmpdir(), 'ziee-test-locks')
const LOCK_TIMEOUT_MS = 180000 // 3 minutes - max test duration

interface PortLock {
  pid: number
  timestamp: number
  vitePort: number
  backendPort: number
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
 * Validate if a lock is still valid
 * A lock is valid if:
 * 1. The process that created it is still alive
 * 2. The lock hasn't expired (timeout fallback)
 */
function isLockValid(lock: PortLock): boolean {
  const now = Date.now()

  // Check if lock is expired (timeout fallback)
  if (now - lock.timestamp > LOCK_TIMEOUT_MS) {
    console.log(`🧹 Lock expired (timestamp: ${lock.timestamp}, now: ${now})`)
    return false
  }

  // Check if process is still alive
  if (!isProcessAlive(lock.pid)) {
    console.log(`🧹 Lock orphaned (PID ${lock.pid} not running)`)
    return false
  }

  return true
}

/**
 * Try to acquire a lock for a specific port pair
 * Returns true if lock was acquired, false if ports are in use
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
        // Lock is stale, remove it
        console.log(`🧹 Removing stale lock: ${lockFile}`)
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
 */
export function cleanupStaleLocks(): void {
  if (!existsSync(LOCK_DIR)) {
    return
  }

  console.log('🧹 Cleaning up stale port locks...')

  const lockFiles = readdirSync(LOCK_DIR).filter(file => file.endsWith('.lock'))
  let removed = 0
  let kept = 0

  for (const lockFile of lockFiles) {
    const lockPath = resolve(LOCK_DIR, lockFile)

    try {
      const lock: PortLock = JSON.parse(readFileSync(lockPath, 'utf-8'))

      if (!isLockValid(lock)) {
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
