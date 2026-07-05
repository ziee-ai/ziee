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
const MANIFEST = [
  { file: 'auth.json', pointer: 'me', schema: 'MeResponse' },
  { file: 'llm-providers.json', pointer: 'providers', schema: 'LlmProviderListResponse' },
  { file: 'llm-providers.json', pointer: 'modelsByProvider.*', schema: 'LlmModelListResponse' },
  { file: 'llm-providers.json', pointer: 'groups', schema: 'GroupListResponse' },
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

function main() {
  const openapi = JSON.parse(fs.readFileSync(OPENAPI, 'utf8'))
  // strict:false → tolerate the OpenAPI 3.0 dialect (`nullable`, `example`,
  // unknown formats). Fixtures are null-stripped at record time, so `nullable`
  // semantics don't affect validation.
  const ajv = new Ajv({ strict: false, allErrors: true, validateFormats: false })
  ajv.addSchema({ $id: 'oa', components: openapi.components })

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

  console.log(`\n${checked} value(s) checked, ${failures} failure(s).`)
  if (failures > 0) process.exit(1)
}

main()
