// `node --test` resolver entrypoint (used by `test:unit` via `--import`).
// Registers node-test-hooks.mjs, which maps `@/…` specifiers to real `src/…`
// files so unit specs can import aliased modules, and stubs the two
// browser-coupled boundaries (`@/core/{module-system,events}`) that the store
// proxy factory imports but never calls. The proxy/React/zustand stay real.
import { register } from 'node:module'
register('./node-test-hooks.mjs', import.meta.url)
