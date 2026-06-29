import * as http from 'node:http'
import * as crypto from 'node:crypto'
import { AddressInfo } from 'node:net'

/**
 * In-process Node mock MCP server (HTTP / Streamable HTTP transport) for
 * E2E sampling tests. Mirrors `tests/mcp/mock_sampling_server.rs` but uses
 * Node's built-in `http` module — no extra deps.
 *
 * Protocol surface:
 *  - POST /mcp
 *      method=initialize          → returns capabilities + protocolVersion
 *      method=notifications/*     → 202
 *      method=tools/list          → exposes one `research` tool
 *      method=tools/call          → opens an SSE stream that emits two
 *                                   `sampling/createMessage` server-initiated
 *                                   requests, waits for the client's
 *                                   responses (via separate POSTs with no
 *                                   `method` field), then emits the final
 *                                   tool result
 *      no method, has result/err  → response to a server-initiated request;
 *                                   matched to a pending sampling-request id
 *
 * Behavior is async — the SSE stream stays open across multiple HTTP POSTs
 * to coordinate the sampling roundtrip.
 *
 * Usage:
 *   const mock = await MockSamplingServer.start()
 *   // ... register mock.url() as an MCP server, send chat message ...
 *   expect(mock.samplingCallCount()).toBe(2)
 *   await mock.dispose()
 */

export class MockSamplingServer {
  private server: http.Server
  private port: number
  private sessionId: string
  private pendingSampling: Map<number, (value: unknown) => void> = new Map()
  private nextServerRequestId = 100
  private _samplingCallCount = 0
  private _samplingResults: unknown[] = []
  private samplingResponseTimeoutMs = 30_000

  private constructor(server: http.Server, port: number) {
    this.server = server
    this.port = port
    this.sessionId = `mock-${crypto.randomBytes(4).toString('hex')}`
  }

  static async start(): Promise<MockSamplingServer> {
    const server = http.createServer()
    const port = await new Promise<number>((resolve, reject) => {
      server.listen(0, '127.0.0.1', () => {
        const addr = server.address() as AddressInfo
        resolve(addr.port)
      })
      server.on('error', reject)
    })

    const mock = new MockSamplingServer(server, port)
    server.on('request', (req, res) => mock.handleRequest(req, res))
    return mock
  }

  url(): string {
    return `http://127.0.0.1:${this.port}/mcp`
  }

  /** Total number of completed sampling roundtrips (one per response received). */
  samplingCallCount(): number {
    return this._samplingCallCount
  }

  /** Each sampling result text the client returned, in order. */
  samplingResults(): unknown[] {
    return [...this._samplingResults]
  }

  async dispose(): Promise<void> {
    // Resolve any still-pending sampling waiters so the SSE handlers don't hang.
    for (const resolver of this.pendingSampling.values()) {
      resolver({ __aborted: true })
    }
    this.pendingSampling.clear()
    await new Promise<void>(resolve => this.server.close(() => resolve()))
  }

  // ────────────────────────────────────────────────────────────────────────

  private async handleRequest(req: http.IncomingMessage, res: http.ServerResponse) {
    if (req.method !== 'POST') {
      res.writeHead(405)
      res.end()
      return
    }

    const body = await readJsonBody(req)

    // Response to a server-initiated request: has result/error, no method.
    if (!body.method && (body.result !== undefined || body.error !== undefined)) {
      const id = typeof body.id === 'number' ? body.id : -1
      const resolver = this.pendingSampling.get(id)
      if (resolver) {
        this.pendingSampling.delete(id)
        resolver(body.result ?? body.error)
        this._samplingCallCount++
        this._samplingResults.push(body.result)
      }
      res.writeHead(202)
      res.end()
      return
    }

    switch (body.method) {
      case 'initialize':
        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id,
          result: {
            protocolVersion: '2025-11-25',
            capabilities: { tools: {}, sampling: {} },
            serverInfo: { name: 'mock-sampling', version: '0.0.1' },
          },
        }, { 'MCP-Session-Id': this.sessionId })
        return

      case 'tools/list':
        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id,
          result: {
            tools: [{
              name: 'research',
              description: 'Research a query using two sequential LLM sampling calls. ' +
                'Use this for any factual question.',
              inputSchema: {
                type: 'object',
                properties: { query: { type: 'string', description: 'The research question' } },
                required: ['query'],
              },
            }],
          },
        })
        return

      case 'tools/call':
        await this.handleToolCallSse(body, res)
        return

      default:
        if (typeof body.method === 'string' && body.method.startsWith('notifications/')) {
          res.writeHead(202)
          res.end()
          return
        }
        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id ?? null,
          error: { code: -32601, message: `Method not found: ${body.method}` },
        })
    }
  }

  private async handleToolCallSse(
    body: Record<string, unknown>,
    res: http.ServerResponse,
  ): Promise<void> {
    const toolCallId = body.id
    const params = (body.params ?? {}) as Record<string, unknown>
    const args = (params.arguments ?? {}) as Record<string, unknown>
    const query = typeof args.query === 'string' ? args.query : 'unknown'

    res.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'MCP-Session-Id': this.sessionId,
    })

    const writeEvent = (data: unknown) => {
      res.write(`data: ${JSON.stringify(data)}\n\n`)
    }

    // ── Sampling request #1: answer the question ──
    const id1 = this.nextServerRequestId++
    writeEvent({
      jsonrpc: '2.0',
      id: id1,
      method: 'sampling/createMessage',
      params: {
        messages: [{
          role: 'user',
          content: { type: 'text', text: `Answer this question: ${query}` },
        }],
        maxTokens: 500,
      },
    })

    const r1 = await this.awaitSamplingResponse(id1)
    if (r1 === '__timeout' || (r1 as Record<string, unknown>)?.__aborted) {
      writeEvent({
        jsonrpc: '2.0',
        id: null,
        error: { code: -32000, message: 'sampling timeout/abort #1' },
      })
      res.end()
      return
    }
    const text1 = extractText(r1)

    // ── Sampling request #2: summarize ──
    const id2 = this.nextServerRequestId++
    writeEvent({
      jsonrpc: '2.0',
      id: id2,
      method: 'sampling/createMessage',
      params: {
        messages: [{
          role: 'user',
          content: { type: 'text', text: `Summarize in one sentence: ${text1}` },
        }],
        maxTokens: 100,
      },
    })

    const r2 = await this.awaitSamplingResponse(id2)
    if (r2 === '__timeout' || (r2 as Record<string, unknown>)?.__aborted) {
      writeEvent({
        jsonrpc: '2.0',
        id: null,
        error: { code: -32000, message: 'sampling timeout/abort #2' },
      })
      res.end()
      return
    }
    const text2 = extractText(r2)

    // ── Final tool result ──
    writeEvent({
      jsonrpc: '2.0',
      id: toolCallId,
      result: {
        content: [{ type: 'text', text: text2 }],
        isError: false,
      },
    })
    res.end()
  }

  private awaitSamplingResponse(id: number): Promise<unknown> {
    return new Promise(resolve => {
      this.pendingSampling.set(id, resolve)
      setTimeout(() => {
        if (this.pendingSampling.has(id)) {
          this.pendingSampling.delete(id)
          resolve('__timeout')
        }
      }, this.samplingResponseTimeoutMs)
    })
  }

  private respondJson(
    res: http.ServerResponse,
    body: unknown,
    extraHeaders: Record<string, string> = {},
  ): void {
    res.writeHead(200, {
      'Content-Type': 'application/json',
      ...extraHeaders,
    })
    res.end(JSON.stringify(body))
  }
}

// ──────────────────────────────────────────────────────────────────────────

async function readJsonBody(req: http.IncomingMessage): Promise<Record<string, unknown>> {
  const chunks: Buffer[] = []
  for await (const chunk of req) {
    chunks.push(chunk as Buffer)
  }
  const text = Buffer.concat(chunks).toString('utf8')
  if (!text) return {}
  try {
    return JSON.parse(text)
  } catch {
    return {}
  }
}

function extractText(samplingResult: unknown): string {
  const r = samplingResult as { content?: { text?: string } } | null
  return r?.content?.text ?? ''
}
