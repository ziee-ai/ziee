import * as http from 'node:http'
import * as crypto from 'node:crypto'
import { AddressInfo } from 'node:net'

/**
 * In-process Node mock MCP server (HTTP / Streamable HTTP transport)
 * that exposes a single `get_file_link` tool returning a fake
 * `resource_link` content block. Lets LLM-gated E2E tests drive the
 * end-to-end flow without a real sandbox.
 *
 * Protocol surface:
 *  - POST /mcp
 *      method=initialize          → returns capabilities + protocolVersion
 *      method=notifications/*     → 202
 *      method=tools/list          → exposes one `get_file_link` tool
 *      method=tools/call          → returns content[] containing a single
 *                                   {type:'resource_link', ...} block whose
 *                                   uri/name/mime are taken from the tool
 *                                   input (`name`, `mime_type`, `uri`).
 *
 * The point of this mock is NOT to verify the MCP wire protocol — that's
 * the sampling mock's job. It's to give the LLM a tool whose name + input
 * schema make sense ("get_file_link returns a file URL the user can see")
 * so the LLM actually invokes it without prompting gymnastics.
 *
 * Usage:
 *   const mock = await MockResourceLinkServer.start({ baseUrl: testInfra.baseURL })
 *   // ... register mock.url() as a system MCP server, send chat message ...
 *   expect(mock.toolCallCount()).toBeGreaterThan(0)
 *   await mock.dispose()
 */

export interface MockResourceLinkServerOptions {
  /** Base URL of the test backend — passed in for parity with the
   *  sampling mock, currently unused by this mock's request handlers
   *  (the test intercepts the resource_link URL via page.route). */
  baseUrl: string
}

export class MockResourceLinkServer {
  private server: http.Server
  private port: number
  private sessionId: string
  private _toolCallCount = 0
  private _toolInputs: unknown[] = []

  private constructor(server: http.Server, port: number) {
    this.server = server
    this.port = port
    this.sessionId = `mock-rl-${crypto.randomBytes(4).toString('hex')}`
  }

  static async start(_options: MockResourceLinkServerOptions): Promise<MockResourceLinkServer> {
    const server = http.createServer()
    const port = await new Promise<number>((resolve, reject) => {
      server.listen(0, '127.0.0.1', () => {
        const addr = server.address() as AddressInfo
        resolve(addr.port)
      })
      server.on('error', reject)
    })

    const mock = new MockResourceLinkServer(server, port)
    server.on('request', (req, res) => mock.handleRequest(req, res))
    return mock
  }

  url(): string {
    return `http://127.0.0.1:${this.port}/mcp`
  }

  toolCallCount(): number {
    return this._toolCallCount
  }

  toolInputs(): unknown[] {
    return [...this._toolInputs]
  }

  async dispose(): Promise<void> {
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

    switch (body.method) {
      case 'initialize':
        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id,
          result: {
            protocolVersion: '2025-11-25',
            capabilities: { tools: {} },
            serverInfo: { name: 'mock-resource-link', version: '0.0.1' },
          },
        }, { 'MCP-Session-Id': this.sessionId })
        return

      case 'tools/list':
        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id,
          result: {
            tools: [{
              name: 'get_file_link',
              description:
                'Return a URL the user can view for a file you produced. ' +
                'Use this whenever you want the user to SEE a file — the URL ' +
                'is rendered inline in the chat as the appropriate preview ' +
                '(image for PNG, table for CSV, rendered markdown for MD, etc.).',
              inputSchema: {
                type: 'object',
                required: ['name', 'mime_type'],
                properties: {
                  name: {
                    type: 'string',
                    description: 'Filename to show the user, e.g. "plot.png" or "data.csv".',
                  },
                  mime_type: {
                    type: 'string',
                    description: 'MIME type of the file, e.g. "image/png".',
                  },
                  uri: {
                    type: 'string',
                    description: 'Optional URI override. Defaults to /api/files/mock/<name>.',
                  },
                },
              },
            }],
          },
        })
        return

      case 'tools/call': {
        const params = (body.params ?? {}) as Record<string, unknown>
        const args = (params.arguments ?? {}) as Record<string, unknown>
        this._toolCallCount += 1
        this._toolInputs.push(args)

        const name = (args.name as string) ?? 'untitled'
        const mimeType = (args.mime_type as string) ?? 'application/octet-stream'
        const uri = (args.uri as string) ?? `/api/files/mock/${encodeURIComponent(name)}`

        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id,
          result: {
            content: [
              {
                type: 'resource_link',
                uri,
                name,
                mimeType: mimeType,
              },
            ],
            isError: false,
          },
        })
        return
      }

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
