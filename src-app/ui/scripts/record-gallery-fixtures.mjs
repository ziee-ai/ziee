#!/usr/bin/env node
/**
 * Record REAL server responses as gallery fixture cassettes (record/replay).
 *
 * Boots a ziee server against a throwaway embedded-Postgres + data dir, runs
 * first-run setup with a known admin, optionally loads
 * `server/seeds/showcase/showcase.sql`, then hits each endpoint the gallery
 * needs and saves the ACTUAL JSON into `src/dev/gallery/fixtures/recorded/`.
 *
 * The gallery replays these responses, so fixture shapes are correct by
 * construction (layer 2 of the 3-layer fixture-correctness plan). The typed
 * `fixtures/*.ts` wrappers (layer 1) and the ajv contract test (layer 3) catch
 * any drift.
 *
 * Usage:
 *   node scripts/record-gallery-fixtures.mjs [--only=llm-providers,files,...]
 *
 * Env:
 *   ZIEE_BINARY   path to a prebuilt `ziee` binary (skips cargo build)
 *   KEEP_SERVER=1 leave the server + DB up after recording (debugging)
 */
import { spawn, execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import net from 'node:net'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const UI_DIR = path.resolve(__dirname, '..')
const SERVER_DIR = path.resolve(UI_DIR, '../server')
const OUT_DIR = path.resolve(UI_DIR, 'src/dev/gallery/fixtures/recorded')

const ADMIN = { username: 'admin', email: 'admin@gallery.dev', password: 'gallery-admin-pw-123' }

const args = process.argv.slice(2)
const onlyArg = args.find(a => a.startsWith('--only='))
const only = onlyArg ? onlyArg.slice('--only='.length).split(',') : null

const log = (...a) => console.log('[record]', ...a)

/**
 * Deep-remove `null`-valued keys. The server serializes `Option::None` optional
 * fields as `null`, but the generated api-client types mark them `?:` (i.e.
 * `T | undefined`, not `| null`). `null` and "absent" are indistinguishable to
 * the client for these fields, so normalizing null→absent keeps the cassette a
 * faithful replay while making it assignable to the response types (a genuinely
 * required `T | null` field, if any, would then fail `tsc` as missing — the
 * drift guard still holds). Arrays/values are preserved.
 */
function stripNulls(value) {
  if (Array.isArray(value)) return value.map(stripNulls)
  if (value && typeof value === 'object') {
    const out = {}
    for (const [k, v] of Object.entries(value)) {
      if (v === null) continue
      out[k] = stripNulls(v)
    }
    return out
  }
  return value
}

function freePort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer()
    srv.unref()
    srv.on('error', reject)
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address()
      srv.close(() => resolve(port))
    })
  })
}

async function waitFor(fn, { tries = 120, delayMs = 1000 } = {}) {
  let last
  for (let i = 0; i < tries; i++) {
    try {
      return await fn()
    } catch (e) {
      last = e
      await new Promise(r => setTimeout(r, delayMs))
    }
  }
  throw last
}

async function getJson(url, token) {
  const res = await fetch(url, {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  })
  if (!res.ok) throw new Error(`GET ${url} -> ${res.status}`)
  return res.json()
}

async function postJson(url, body, token) {
  const res = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  })
  const text = await res.text()
  let json
  try {
    json = JSON.parse(text)
  } catch {
    json = text
  }
  return { ok: res.ok, status: res.status, json }
}

/** Parse `ApiEndpoints` from the generated types.ts → [{key, method, path}]. */
function parseEndpoints() {
  const src = fs.readFileSync(path.join(UI_DIR, 'src/api-client/types.ts'), 'utf8')
  const start = src.indexOf('export const ApiEndpoints')
  const end = src.indexOf('} as const', start)
  const block = src.slice(start, end)
  const re = /'([^']+)':\s*'(GET|POST|PUT|DELETE) ([^']+)'/g
  const out = []
  let m
  while ((m = re.exec(block))) out.push({ key: m[1], method: m[2], path: m[3] })
  return out
}

// Endpoints unsafe/pointless to crawl: SSE/streams, blob exports, per-request
// JSON-RPC (trailing /mcp), proxies. Regex so `/mcp$` doesn't nuke `/api/mcp/*`.
const CRAWL_SKIP = [
  /\/subscribe/,
  /\/stream/,
  /\/export/,
  /\/download/,
  /\/mcp$/, // built-in MCP JSON-RPC endpoints only (NOT /api/mcp/servers)
  /\/local-llm\/v1/,
  /\/setup\/status/,
  /\/health/,
  /\/auth\/me/, // recorded separately
]

function resolveBinary() {
  if (process.env.ZIEE_BINARY && fs.existsSync(process.env.ZIEE_BINARY)) {
    return process.env.ZIEE_BINARY
  }
  // Fall back to a sibling worktree's compiled debug binary if present, else
  // build from this worktree.
  log('no ZIEE_BINARY; building server (this can take several minutes)…')
  execFileSync('cargo', ['build', '-p', 'ziee'], {
    cwd: SERVER_DIR,
    stdio: 'inherit',
  })
  return path.resolve(SERVER_DIR, '../target/debug/ziee')
}

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true })
  const runRoot = fs.mkdtempSync(path.join('/data/pbya/ziee/tmp', 'gallery-record-'))
  const appPort = await freePort()
  const pgPort = await freePort()
  const configPath = path.join(runRoot, 'record.yaml')
  const config = `
app:
  data_dir: "${runRoot}/app-data"
postgresql:
  use_embedded: true
  embedded:
    version: "18.3.0"
    port: ${pgPort}
    bind_address: "127.0.0.1"
    username: "postgres"
    password: "password"
    database: "postgres"
    installation_dir: "${runRoot}/postgres"
    data_dir: "${runRoot}/postgres-data"
    timezone: "UTC"
    log_timezone: "UTC"
    logging:
      collector: true
      directory: "log"
      filename: "postgresql-%Y-%m-%d_%H%M%S.log"
      statement: "all"
  pool: { max_connections: 10, min_connections: 1, acquire_timeout_secs: 5, idle_timeout_secs: 30, max_lifetime_secs: 300 }
server:
  host: "127.0.0.1"
  port: ${appPort}
  api_prefix: "/api"
  rate_limit: { enabled: false }
  cors:
    allow_origins: ["http://localhost:${appPort}"]
    allow_methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
    allow_headers: ["Content-Type", "Authorization"]
logging: { level: "warn", format: "pretty" }
jwt:
  secret: "gallery-record-secret-please-do-not-use-in-prod-000000000000"
  issuer: "ziee"
  audience: "ziee-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: 30
code_sandbox: { enabled: false, rootfs_path: "./none", cgroup_parent: "" }
update_check: { enabled: false }
`
  fs.writeFileSync(configPath, config)

  const binary = resolveBinary()
  log(`booting ${binary} on :${appPort} (pg :${pgPort}) — data ${runRoot}`)
  const child = spawn(binary, [], {
    cwd: SERVER_DIR,
    env: { ...process.env, CONFIG_FILE: configPath },
    stdio: ['ignore', 'inherit', 'inherit'],
  })

  const base = `http://127.0.0.1:${appPort}`
  const cleanup = () => {
    if (process.env.KEEP_SERVER) {
      log(`KEEP_SERVER set — leaving server up on ${base}, data ${runRoot}`)
      return
    }
    try {
      child.kill('SIGTERM')
    } catch {}
    try {
      fs.rmSync(runRoot, { recursive: true, force: true })
    } catch {}
  }
  process.on('exit', cleanup)
  process.on('SIGINT', () => {
    cleanup()
    process.exit(1)
  })

  try {
    // 1. Wait for boot (setup/status reachable).
    const status = await waitFor(() => getJson(`${base}/api/app/setup/status`))
    log('server up; needs_setup =', status.needs_setup)

    // 2. First-run setup (known admin).
    if (status.needs_setup) {
      const r = await postJson(`${base}/api/app/setup/admin`, ADMIN)
      if (!r.ok) throw new Error(`setup/admin failed: ${r.status} ${JSON.stringify(r.json)}`)
      log('admin created')
    }

    // 3. Login → token.
    const login = await postJson(`${base}/api/auth/login`, {
      username: ADMIN.username,
      password: ADMIN.password,
    })
    if (!login.ok) throw new Error(`login failed: ${login.status} ${JSON.stringify(login.json)}`)
    const token = login.json.access_token
    if (!token) throw new Error('no access_token in login response')
    log('logged in')

    // Load showcase.sql (chat conversation data) so the chat recorder can record
    // real multi-state conversations. Owner = the admin we just created.
    const me = await getJson(`${base}/api/auth/me`, token)
    const ownerId = me.user.id
    try {
      execFileSync(
        'psql',
        [
          `postgresql://postgres:password@127.0.0.1:${pgPort}/postgres`,
          // NOT ON_ERROR_STOP: a few rows (e.g. mcp_tool_calls referencing a
          // server_id absent on a fresh boot) fail their FK — skip them and keep
          // loading the conversations/messages/branches we need.
          '-v', `owner=${ownerId}`,
          '-f', path.join(SERVER_DIR, 'seeds/showcase/showcase.sql'),
        ],
        { stdio: ['ignore', 'ignore', 'ignore'] },
      )
      log('showcase.sql loaded (best-effort)')
    } catch (e) {
      log(`showcase load failed (chat cassettes will be empty): ${e.message}`)
    }

    // 4. Record. Each recorder returns { file, data }.
    const recorders = {
      auth: async () => {
        const me = await getJson(`${base}/api/auth/me`, token)
        return { 'auth.json': { me } }
      },
      // Chat multi-state: record the conversation-detail endpoints for the
      // showcase conversations (rich/tool-calls/elicitation), each keyed by id
      // so the gallery can render distinct conversation states in isolation.
      chat: async () => {
        const conversations = await getJson(
          `${base}/api/conversations?page=1&per_page=100`,
          token,
        )
        const convIds = (conversations.conversations ?? []).map(c => c.id)
        const byId = {}
        for (const id of convIds) {
          try {
            byId[id] = {
              conversation: await getJson(`${base}/api/conversations/${id}`, token),
              messages: await getJson(`${base}/api/conversations/${id}/messages`, token),
              branches: await getJson(`${base}/api/conversations/${id}/branches`, token),
            }
          } catch (e) {
            log(`  chat ${id} -> ${e.message}`)
          }
        }
        log(`  recorded ${Object.keys(byId).length} conversation(s)`)
        return { 'chat.json': { conversations, byId } }
      },
      // Broad crawl: every SAFE paramless GET → its live response. Feeds the
      // generic crawl cassette so list/settings pages across all modules
      // populate. Path-param detail endpoints stay in per-module fixtures.
      crawl: async () => {
        const eps = parseEndpoints().filter(
          e =>
            e.method === 'GET' &&
            !e.path.includes('{') &&
            !CRAWL_SKIP.some(rx => rx.test(e.path)),
        )
        const crawl = {}
        for (const ep of eps) {
          // default page/per_page for paginated ones (harmless on the rest).
          const url = `${base}${ep.path}${ep.path.includes('?') ? '&' : '?'}page=1&per_page=100`
          try {
            const res = await fetch(url, { headers: { Authorization: `Bearer ${token}` } })
            if (!res.ok) {
              log(`  crawl skip ${ep.key} -> ${res.status}`)
              continue
            }
            const ct = res.headers.get('content-type') || ''
            if (!ct.includes('application/json')) {
              log(`  crawl skip ${ep.key} -> ${ct || 'no-ct'}`)
              continue
            }
            crawl[ep.key] = await res.json()
          } catch (e) {
            log(`  crawl error ${ep.key} -> ${e.message}`)
          }
        }
        log(`  crawled ${Object.keys(crawl).length}/${eps.length} paramless GET endpoints`)
        return { 'crawl.json': crawl }
      },
      'llm-providers': async () => {
        // A fresh server seeds the built-in providers DISABLED with 0 models.
        // Populate a realistic state THROUGH the real API (enable a few
        // providers + create models via the real create endpoint) so the
        // recorded list has genuine, contract-correct model shapes.
        const list0 = await getJson(`${base}/api/llm-providers?page=1&per_page=50`, token)
        const byType = Object.fromEntries((list0.providers ?? []).map(p => [p.provider_type, p]))

        const remoteModel = (name, display_name, caps) => ({
          name,
          display_name,
          enabled: true,
          engine_type: 'mistralrs',
          file_format: 'safetensors',
          capabilities: {
            chat: true,
            tools: caps?.tools ?? false,
            vision: caps?.vision ?? false,
            audio: false,
            code_interpreter: false,
            text_embedding: caps?.text_embedding ?? false,
            image_generator: false,
          },
        })

        const seedPlan = [
          {
            type: 'anthropic',
            base_url: 'https://api.anthropic.com',
            models: [
              remoteModel('claude-opus-4-8', 'Claude Opus 4.8', { tools: true, vision: true }),
              remoteModel('claude-sonnet-5', 'Claude Sonnet 5', { tools: true, vision: true }),
              remoteModel('claude-haiku-4-5', 'Claude Haiku 4.5', { tools: true }),
            ],
          },
          {
            type: 'openai',
            base_url: 'https://api.openai.com/v1',
            models: [
              remoteModel('gpt-4o', 'GPT-4o', { tools: true, vision: true }),
              remoteModel('gpt-4o-mini', 'GPT-4o mini', { tools: true }),
              remoteModel('text-embedding-3-large', 'Embedding 3 Large', { text_embedding: true }),
            ],
          },
          {
            type: 'gemini',
            base_url: 'https://generativelanguage.googleapis.com',
            models: [
              remoteModel('gemini-2.5-pro', 'Gemini 2.5 Pro', { tools: true, vision: true }),
              remoteModel('gemini-2.5-flash', 'Gemini 2.5 Flash', { tools: true }),
            ],
          },
          {
            type: 'deepseek',
            base_url: 'https://api.deepseek.com',
            models: [remoteModel('deepseek-chat', 'DeepSeek Chat', { tools: true })],
          },
        ]

        for (const plan of seedPlan) {
          const p = byType[plan.type]
          if (!p) continue
          const upd = await postJson(
            `${base}/api/llm-providers/${p.id}`,
            { enabled: true, api_key: 'sk-gallery-demo-key', base_url: plan.base_url },
            token,
          )
          if (!upd.ok) log(`  ! enable ${plan.type} -> ${upd.status} ${JSON.stringify(upd.json)}`)
          for (const m of plan.models) {
            const created = await postJson(
              `${base}/api/llm-models`,
              { ...m, provider_id: p.id },
              token,
            )
            if (!created.ok) log(`  ! create model ${m.name} -> ${created.status} ${JSON.stringify(created.json)}`)
          }
        }

        // Now read back the populated list + per-provider models + group data
        // (the provider settings page also renders a group-assignment card).
        const providers = await getJson(`${base}/api/llm-providers?page=1&per_page=50`, token)
        const modelsByProvider = {}
        const groupsByProvider = {}
        for (const p of providers.providers ?? []) {
          modelsByProvider[p.id] = await getJson(
            `${base}/api/llm-models?providerId=${encodeURIComponent(p.id)}&page=1&perPage=100`,
            token,
          )
          groupsByProvider[p.id] = await getJson(
            `${base}/api/llm-providers/${encodeURIComponent(p.id)}/groups`,
            token,
          )
        }
        const groups = await getJson(`${base}/api/groups?page=1&per_page=1000`, token)
        return {
          'llm-providers.json': { providers, modelsByProvider, groups, groupsByProvider },
        }
      },
    }

    const selected = (
      only
        ? Object.entries(recorders).filter(([k]) => only.includes(k) || k === 'auth')
        : Object.entries(recorders)
    ).sort(([a], [b]) => (a === 'crawl' ? 1 : b === 'crawl' ? -1 : 0)) // crawl last

    for (const [name, fn] of selected) {
      log(`recording ${name}…`)
      const files = await fn()
      for (const [file, data] of Object.entries(files)) {
        const dest = path.join(OUT_DIR, file)
        fs.writeFileSync(dest, `${JSON.stringify(stripNulls(data), null, 2)}\n`)
        log(`  wrote ${path.relative(UI_DIR, dest)}`)
      }
    }
    log('done')
  } finally {
    cleanup()
  }
}

main().catch(err => {
  console.error('[record] FAILED:', err)
  process.exit(1)
})
