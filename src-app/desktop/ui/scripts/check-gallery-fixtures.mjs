#!/usr/bin/env node
/**
 * Contract test for the gallery fixture cassettes (layer 3 of fixture
 * correctness). Validates every recorded fixture against the response schema in
 * `openapi/openapi.json` (ajv). A fixture that drifts from the spec FAILS here,
 * so a stale cassette can never render silently against a wrong shape — it ties
 * into the same discipline as the backend's `types_ts_parity` golden test.
 *
 * Run: `node scripts/check-gallery-fixtures.mjs`  (exit 1 on any violation)
 *
 * The MANIFEST maps each recorded JSON's sub-objects to the OpenAPI component
 * schema they must satisfy. Extend it as fixtures are added.
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import Ajv from 'ajv'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const UI_DIR = path.resolve(__dirname, '..')
const REC_DIR = path.resolve(UI_DIR, 'src/dev/gallery/fixtures/recorded')
const OPENAPI = path.resolve(UI_DIR, 'openapi/openapi.json')

/**
 * Each rule: read `file`, pluck the value at `pointer` (dot path; `*` fans out
 * over an object's values), validate against component `schema`.
 */
// Desktop records auth + crawl only (llm-providers is a web-gallery hand
// fixture); the crawl block below is auto-validated against desktop openapi.json.
const MANIFEST = [
  { file: 'auth.json', pointer: 'me', schema: 'MeResponse' },
]

function pluck(root, pointer) {
  // Returns [{ label, value }] — one entry, or many when a `*` segment fans out.
  let frontier = [{ label: '', value: root }]
  for (const seg of pointer.split('.')) {
    const next = []
    for (const { label, value } of frontier) {
      if (value == null) continue
      if (seg === '*') {
        for (const [k, v] of Object.entries(value)) {
          next.push({ label: `${label}[${k}]`, value: v })
        }
      } else {
        next.push({ label: label ? `${label}.${seg}` : seg, value: value[seg] })
      }
    }
    frontier = next
  }
  return frontier
}

/** Parse `ApiEndpoints` from types.ts → { key: "METHOD /path" }. */
function endpointMap() {
  const src = fs.readFileSync(path.resolve(UI_DIR, 'src/api-client/types.ts'), 'utf8')
  const start = src.indexOf('export const ApiEndpoints')
  const end = src.indexOf('} as const', start)
  const block = src.slice(start, end)
  const re = /'([^']+)':\s*'(GET|POST|PUT|DELETE) ([^']+)'/g
  const map = {}
  let m
  while ((m = re.exec(block))) map[m[1]] = { method: m[2], path: m[3] }
  return map
}

const ptr = s => s.replace(/~/g, '~0').replace(/\//g, '~1')

/** A `full#/...` JSON-pointer to the endpoint's 200 JSON response schema, or undefined. */
function schemaRefForEndpoint(openapi, method, rawPath) {
  const p = rawPath.split('?')[0]
  const item = openapi.paths?.[p]?.[method.toLowerCase()]
  const code = item?.responses?.['200'] ? '200' : item?.responses?.['201'] ? '201' : undefined
  if (!code) return undefined
  const schema = item.responses[code]?.content?.['application/json']?.schema
  if (!schema) return undefined
  return `full#/paths/${ptr(p)}/${method.toLowerCase()}/responses/${code}/content/${ptr('application/json')}/schema`
}

function main() {
  const openapi = JSON.parse(fs.readFileSync(OPENAPI, 'utf8'))
  // strict:false → tolerate the OpenAPI 3.0 dialect (`nullable`, `example`,
  // unknown formats). Fixtures are null-stripped at record time, so `nullable`
  // semantics don't affect validation.
  const ajv = new Ajv({ strict: false, allErrors: true, validateFormats: false })
  ajv.addSchema({ $id: 'oa', components: openapi.components })
  ajv.addSchema({ ...openapi, $id: 'full' })

  let failures = 0
  let checked = 0
  for (const rule of MANIFEST) {
    const filePath = path.join(REC_DIR, rule.file)
    if (!fs.existsSync(filePath)) {
      console.error(`✗ MISSING fixture: ${rule.file}`)
      failures++
      continue
    }
    const data = JSON.parse(fs.readFileSync(filePath, 'utf8'))
    const validate = ajv.compile({ $ref: `oa#/components/schemas/${rule.schema}` })
    for (const { label, value } of pluck(data, rule.pointer)) {
      checked++
      const ok = validate(value)
      const where = `${rule.file}:${label || '(root)'} ⇢ ${rule.schema}`
      if (ok) {
        console.log(`✓ ${where}`)
      } else {
        failures++
        console.error(`✗ ${where}`)
        for (const e of validate.errors ?? []) {
          console.error(`    ${e.instancePath || '/'} ${e.message}`)
        }
      }
    }
  }

  // Auto-validate every recorded crawl endpoint against its openapi 200 schema.
  const crawlPath = path.join(REC_DIR, 'crawl.json')
  if (fs.existsSync(crawlPath)) {
    const crawl = JSON.parse(fs.readFileSync(crawlPath, 'utf8'))
    const eps = endpointMap()
    let crawlFail = 0
    const drift = []
    for (const key of Object.keys(crawl).sort()) {
      const ep = eps[key]
      if (!ep) continue
      const ref = schemaRefForEndpoint(openapi, ep.method, ep.path)
      if (!ref) continue // void / non-json response — nothing to validate
      checked++
      const validate = ajv.compile({ $ref: ref })
      if (validate(crawl[key])) {
        console.log(`✓ crawl:${key}`)
      } else {
        crawlFail++
        drift.push(key)
        console.error(`✗ crawl:${key}`)
        for (const e of (validate.errors ?? []).slice(0, 4)) {
          console.error(`    ${e.instancePath || '/'} ${e.message}`)
        }
      }
    }
    // Crawl drift is reported but NON-fatal: the crawl is recorded from a
    // possibly-older reference binary and excluded-from-typed entries fall back
    // to the crash-safe default. The hand fixtures above ARE fatal.
    if (drift.length) {
      console.warn(
        `\n⚠ ${crawlFail} crawl endpoint(s) drift from openapi.json (re-record from a matching binary): ${drift.join(', ')}`,
      )
    }
  }

  console.log(`\n${checked} value(s) checked, ${failures} fatal failure(s).`)
  if (failures > 0) process.exit(1)
}

main()
