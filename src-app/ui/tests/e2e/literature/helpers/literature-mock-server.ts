import * as http from 'node:http'
import * as crypto from 'node:crypto'
import { AddressInfo } from 'node:net'

/**
 * In-process Node mock MCP server (HTTP / Streamable HTTP transport) that
 * exposes a single `literature_search` tool. The tool returns a tool result
 * carrying a typed `structuredContent` payload in the exact shape the
 * production literature screening surface consumes (LitStructured: query +
 * records[] + identified + after_dedup + degraded_sources + completeness).
 *
 * Why a mock MCP server instead of the seeded `mockChatTokenStream` fixture
 * (screening-flow.spec.ts): this lets an LLM-gated test drive the REAL
 * chat → model-decides-to-call-tool → tool_result(structured_content)
 * persisted → LiteratureToolResultCard → screening panel path end to end. The
 * card selects purely by tool NAME (`block.name === 'literature_search'`,
 * LiteratureToolResultCard.tsx:24), not by the built-in server id, so a tool
 * named `literature_search` on this mock renders the same card production
 * renders for the built-in lit_search server — without the built-in being
 * enabled and without live Europe PMC network. Only the upstream data source
 * (the search results) is mocked; the model's tool-call decision, the
 * structured_content persistence, and the panel rendering are all real.
 */

export interface LitRecord {
  doi?: string | null
  pmid?: string | null
  title: string
  abstract_text?: string | null
  authors: string[]
  year?: number | null
  venue?: string | null
  url?: string | null
  source: string
  source_ids: string[]
  cited_by_count?: number | null
  is_preprint: boolean
  relevance: number
}

export interface LitStructured {
  query: string
  records: LitRecord[]
  identified: Record<string, number>
  after_dedup: number
  degraded_sources: string[]
  completeness: { estimate: string; method: string; caveat: string } | null
}

export class LiteratureMockServer {
  private server: http.Server
  private port: number
  private sessionId: string
  private _toolCallCount = 0
  private payload: LitStructured

  private constructor(server: http.Server, port: number, payload: LitStructured) {
    this.server = server
    this.port = port
    this.sessionId = `mock-lit-${crypto.randomBytes(4).toString('hex')}`
    this.payload = payload
  }

  static async start(payload: LitStructured): Promise<LiteratureMockServer> {
    const server = http.createServer()
    const port = await new Promise<number>((resolve, reject) => {
      server.listen(0, '127.0.0.1', () => {
        const addr = server.address() as AddressInfo
        resolve(addr.port)
      })
      server.on('error', reject)
    })
    const mock = new LiteratureMockServer(server, port, payload)
    server.on('request', (req, res) => mock.handleRequest(req, res))
    return mock
  }

  url(): string {
    return `http://127.0.0.1:${this.port}/mcp`
  }

  toolCallCount(): number {
    return this._toolCallCount
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
        this.respondJson(
          res,
          {
            jsonrpc: '2.0',
            id: body.id,
            result: {
              protocolVersion: '2025-11-25',
              capabilities: { tools: {} },
              serverInfo: { name: 'mock-literature', version: '0.0.1' },
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
                name: 'literature_search',
                description:
                  'Search the scholarly literature for papers matching a query. ' +
                  'Returns deduplicated bibliographic records the user can screen. ' +
                  'Call this tool with the user\'s topic as `query`; you do not need ' +
                  'to know specific papers yourself — the tool finds and returns them.',
                inputSchema: {
                  type: 'object',
                  required: ['query'],
                  properties: {
                    query: {
                      type: 'string',
                      description: 'The literature search query / topic.',
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
        const params = (body.params ?? {}) as Record<string, unknown>
        const args = (params.arguments ?? {}) as Record<string, unknown>
        const query = (args.query as string) ?? this.payload.query
        const sc: LitStructured = { ...this.payload, query }
        const digest = `Literature search: "${query}" — ${sc.after_dedup} records after dedup.`

        this.respondJson(res, {
          jsonrpc: '2.0',
          id: body.id,
          result: {
            content: [{ type: 'text', text: digest }],
            structuredContent: sc,
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
    res.writeHead(200, { 'Content-Type': 'application/json', ...extraHeaders })
    res.end(JSON.stringify(body))
  }
}

export function sampleLiteraturePayload(): LitStructured {
  const rec = (doi: string, title: string, year: number): LitRecord => ({
    doi,
    pmid: null,
    title,
    abstract_text: `Abstract for ${title}.`,
    authors: ['Smith J', 'Doe A'],
    year,
    venue: 'Nature',
    url: `https://doi.org/${doi}`,
    source: 'europepmc',
    source_ids: [`europepmc:${doi}`],
    cited_by_count: 12,
    is_preprint: false,
    relevance: 0.92,
  })
  return {
    query: 'CRISPR base editing off-target effects',
    records: [
      rec('10.9/zzz1', 'Base editing reduces off-target effects', 2021),
      rec('10.9/zzz2', 'A second relevant base-editing study', 2022),
    ],
    identified: { europepmc: 2, crossref: 1 },
    after_dedup: 2,
    degraded_sources: [],
    completeness: {
      estimate: 'MODERATE',
      method: 'capture-recapture',
      caveat: 'Adjunct to, not a replacement for, systematic searching.',
    },
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
