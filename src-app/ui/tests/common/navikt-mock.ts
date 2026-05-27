/**
 * Spawn a navikt/mock-oauth2-server in Docker for one E2E test.
 *
 * Mirrors `src-app/server/tests/common/oauth_mock.rs` so our parity
 * test exercises the same provider the Rust integration tests use.
 * Each instance binds a random host port to avoid worker collisions
 * (Playwright workers run in parallel).
 *
 * Usage:
 *   const mock = await startNaviktMock()
 *   try {
 *     // mock.issuerUrl, mock.authorizeUrl, mock.tokenUrl ...
 *   } finally {
 *     await mock.stop()
 *   }
 *
 * Requires Docker available on the host. Throws if `docker` is not
 * found on PATH or the container fails its readiness probe.
 */
import { spawn, execSync } from 'child_process'
import { createServer } from 'net'

const NAVIKT_IMAGE = 'ghcr.io/navikt/mock-oauth2-server:2.1.10'
const READY_TIMEOUT_MS = 30_000
const READY_POLL_MS = 250

export interface NaviktMock {
  /** Container name (for debug + manual cleanup). */
  containerName: string
  /** Host port bound to the container's 8080. */
  port: number
  /** Base URL (`http://127.0.0.1:<port>`). */
  baseUrl: string
  /** OIDC issuer for the `default` issuer config. */
  issuerUrl: string
  /** Authorize endpoint URL. */
  authorizeUrl: string
  /** Token endpoint URL. */
  tokenUrl: string
  /** Cleanup: stop + remove the container. Idempotent. */
  stop: () => Promise<void>
}

function findFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = createServer()
    srv.unref()
    srv.on('error', reject)
    srv.listen(0, '127.0.0.1', () => {
      const addr = srv.address()
      if (typeof addr !== 'object' || addr === null) {
        srv.close()
        reject(new Error('Failed to get free port'))
        return
      }
      const port = addr.port
      srv.close(() => resolve(port))
    })
  })
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms))
}

export async function startNaviktMock(): Promise<NaviktMock> {
  // Verify Docker is available before doing anything else.
  try {
    execSync('docker --version', { stdio: 'pipe' })
  } catch {
    throw new Error(
      'Docker is required for the navikt mock OAuth server but `docker` is not on PATH. ' +
        'Install Docker or skip the parity test.',
    )
  }

  const port = await findFreePort()
  const containerName = `ziee-e2e-navikt-${port}-${Math.random()
    .toString(36)
    .slice(2, 8)}`

  // `docker run -d` to detach; image is small and starts fast.
  spawn(
    'docker',
    [
      'run',
      '--rm',
      '-d',
      '--name',
      containerName,
      '-p',
      `${port}:8080`,
      NAVIKT_IMAGE,
    ],
    { stdio: 'pipe' },
  )

  const baseUrl = `http://127.0.0.1:${port}`
  const issuerUrl = `${baseUrl}/default`
  const wellKnown = `${issuerUrl}${'/.well-known/openid-configuration'}`

  // Poll for readiness — navikt doesn't log a "ready" line; wait
  // until the discovery endpoint returns 200.
  const deadline = Date.now() + READY_TIMEOUT_MS
  let lastErr: any
  while (Date.now() < deadline) {
    try {
      const resp = await fetch(wellKnown, {
        signal: AbortSignal.timeout(2000),
      })
      if (resp.ok) {
        return {
          containerName,
          port,
          baseUrl,
          issuerUrl,
          authorizeUrl: `${issuerUrl}/authorize`,
          tokenUrl: `${issuerUrl}/token`,
          stop: async () => stopContainer(containerName),
        }
      }
    } catch (e) {
      lastErr = e
    }
    await sleep(READY_POLL_MS)
  }

  // Readiness probe failed — clean up and throw.
  await stopContainer(containerName)
  throw new Error(
    `navikt mock OAuth server didn't become ready within ${READY_TIMEOUT_MS}ms (port=${port}). Last error: ${lastErr}`,
  )
}

async function stopContainer(name: string): Promise<void> {
  try {
    execSync(`docker stop ${name}`, { stdio: 'pipe', timeout: 10_000 })
  } catch {
    // Container already gone or never started — best-effort cleanup.
  }
}
