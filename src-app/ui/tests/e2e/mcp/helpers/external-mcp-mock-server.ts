import * as http from 'node:http'
import * as os from 'node:os'
import * as crypto from 'node:crypto'
import { AddressInfo } from 'node:net'

/**
 * In-process Node mock MCP server (HTTP / Streamable-HTTP transport) that binds to
 * ALL interfaces (`0.0.0.0`) and advertises its URL on a **non-loopback** local
 * IPv4 address. This is what distinguishes it from `MockResourceLinkServer` (which
 * binds `127.0.0.1` and so resolves `dest_host = None`): the backend's
 * `resolve_dest_host()` returns `Some(host)` only for a NON-loopback http(s) host,
 * so a server registered at `http://<rfc1918-ip>:<port>/mcp` makes the external
 * approval card render its full data-egress disclosure (`approval-dest-host`).
 *
 * The single advertised tool has a long, exact description (so the approval card's
 * `approval-tool-description` has verbatim text to show) and takes a `query` arg
 * (so `approval-tool-args` shows the concrete arguments the model chose).
 *
 * A non-loopback local interface IP is reachable from the backend on the SAME box
 * (the kernel routes a packet to a local interface IP back to itself), and RFC1918
 * addresses pass the backend's `MCP_USER` SSRF policy — so the real backend
 * connects, lists the tool, and resolves the host without any weakening.
 */
export class ExternalMcpMockServer {
  private server: http.Server
  private host: string
  private port: number
  private sessionId: string
  private _toolCallCount = 0

  private constructor(server: http.Server, host: string, port: number) {
    this.server = server
    this.host = host
    this.port = port
    this.sessionId = `mock-ext-${crypto.randomBytes(4).toString('hex')}`
  }

  /** First non-internal IPv4 address of a local interface (an RFC1918 address on
   *  this host). Throws if the box has only loopback (then TEST-181 is genuinely
   *  un-runnable here and should report BLOCKED). */
  static nonLoopbackIpv4(): string {
    for (const addrs of Object.values(os.networkInterfaces())) {
      for (const a of addrs ?? []) {
        if (a.family === 'IPv4' && !a.internal) return a.address
      }
    }
    throw new Error('no non-loopback IPv4 interface available (TEST-181 harness limit)')
  }

  static async start(): Promise<ExternalMcpMockServer> {
    const host = ExternalMcpMockServer.nonLoopbackIpv4()
    const server = http.createServer()
    const port = await new Promise<number>((resolve, reject) => {
      // Bind ALL interfaces so the advertised non-loopback IP is reachable.
      server.listen(0, '0.0.0.0', () => resolve((server.address() as AddressInfo).port))
      server.on('error', reject)
    })
    const mock = new ExternalMcpMockServer(server, host, port)
    server.on('request', (req, res) => mock.handleRequest(req, res))
    return mock
  }

  /** The registerable non-loopback MCP URL (`http://<rfc1918-ip>:<port>/mcp`). */
  url(): string {
    return `http://${this.host}:${this.port}/mcp`
  }

  destHost(): string {
    return this.host
  }

  toolCallCount(): number {
    return this._toolCallCount
  }

  async dispose(): Promise<void> {
    await new Promise<void>(resolve => this.server.close(() => resolve()))
  }

  // The exact, verbatim tool description the approval card must render untruncated.
  static readonly TOOL_DESCRIPTION =
    'Query the external partner knowledge service for a topic and return a short ' +
    'text answer. This calls a THIRD-PARTY hosted endpoint over the network with the ' +
    'exact query string you pass — the operator reviewing this approval is deciding ' +
    'whether that data may egress to the destination host, so the full description ' +
    'and concrete arguments are shown verbatim and are never summarized or truncated.'

  static readonly TOOL_NAME = 'partner_lookup'

  private async handleRequest(req: http.IncomingMessage, res: http.ServerResponse) {
    if (req.method !== 'POST') {
      res.writeHead(405)
      res.end()
      return
    }
    const body = await readJsonBody(req)
    switch (body.method) {
      case 'initialize':
        this.respondJson(
          res,
          {
            jsonrpc: '2.0',
            id: body.id,
            result: {
              protocolVersion: '2025-11-25',
              capabilities: { tools: {} },
              serverInfo: { name: 'external-partner', version: '0.0.1' },
            },
          },
          { 'MCP-Session-Id': this.sessionId },
        )
        return
      case 'tools/list':
        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id,
          result: {
            tools: [
              {
                name: ExternalMcpMockServer.TOOL_NAME,
                description: ExternalMcpMockServer.TOOL_DESCRIPTION,
                inputSchema: {
                  type: 'object',
                  required: ['query'],
                  properties: {
                    query: {
                      type: 'string',
                      description: 'The topic to look up with the external partner service.',
                    },
                  },
                  additionalProperties: false,
                },
              },
            ],
          },
        })
        return
      case 'tools/call': {
        this._toolCallCount += 1
        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id,
          result: { content: [{ type: 'text', text: 'partner result: ok' }], isError: false },
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
    res.writeHead(200, { 'Content-Type': 'application/json', ...extraHeaders })
    res.end(JSON.stringify(body))
  }
}

async function readJsonBody(req: http.IncomingMessage): Promise<Record<string, unknown>> {
  const chunks: Buffer[] = []
  for await (const chunk of req) chunks.push(chunk as Buffer)
  const text = Buffer.concat(chunks).toString('utf8')
  if (!text) return {}
  try {
    return JSON.parse(text)
  } catch {
    return {}
  }
}
