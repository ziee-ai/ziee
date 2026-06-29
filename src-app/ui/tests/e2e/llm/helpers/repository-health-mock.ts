import * as http from 'node:http'
import { AddressInfo } from 'node:net'

/**
 * Tiny in-process Node HTTP mock that stands in for the upstream
 * HuggingFace / GitHub auth-test endpoint. The LLM repository
 * connection-health probe runs server-side (Rust → reqwest), so
 * `page.route()` can't intercept it; we must give the backend a
 * real URL it can connect to.
 *
 * Behavior is mode-switchable mid-test so a single mock can drive
 * the "first try 401, then 200" flow used in the drawer
 * save-then-probe spec.
 *
 *   const mock = await RepoHealthMock.start()
 *   mock.respondWith(401)
 *   // ... drive the UI ...
 *   mock.respondWith(200)
 *   await mock.dispose()
 *
 * The mock listens on 127.0.0.1 with an OS-assigned port; URLs are
 * exposed via `url()`.
 */

export type MockMode = 200 | 401

export class RepoHealthMock {
  private server: http.Server
  private port: number
  private mode: MockMode = 200
  private requestCount = 0

  private constructor(server: http.Server, port: number) {
    this.server = server
    this.port = port
  }

  static async start(initialMode: MockMode = 200): Promise<RepoHealthMock> {
    const server = http.createServer()
    const port = await new Promise<number>((resolve, reject) => {
      server.listen(0, '127.0.0.1', () => {
        const addr = server.address() as AddressInfo
        resolve(addr.port)
      })
      server.on('error', reject)
    })
    const mock = new RepoHealthMock(server, port)
    mock.mode = initialMode
    server.on('request', (_req, res) => mock.handleRequest(res))
    return mock
  }

  /** Hand back the URL the backend should probe (and the row's `url`). */
  url(): string {
    return `http://127.0.0.1:${this.port}/api/test`
  }

  /** Swap the response status mid-test. */
  respondWith(mode: MockMode) {
    this.mode = mode
  }

  /** How many requests the mock has fielded so far. */
  count(): number {
    return this.requestCount
  }

  async dispose(): Promise<void> {
    await new Promise<void>(resolve => {
      this.server.close(() => resolve())
    })
  }

  private handleRequest(res: http.ServerResponse) {
    this.requestCount += 1
    if (this.mode === 401) {
      res.writeHead(401, { 'Content-Type': 'text/plain' })
      res.end('Unauthorized')
    } else {
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end('{}')
    }
  }
}
