import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  isEligible,
  coreEntries,
  orderByDependencies,
  entryForPath,
  type ModuleManifestEntry,
} from '../../../../sdk/packages/framework/src/module-system/manifest.ts'
// @ts-expect-error — JS plugin, no types
import { extractModule } from '../../plugins/vite-plugin-module-manifest.js'
import type { ModuleLoadContext } from '../../../../sdk/packages/framework/src/module-system/types.ts'

const ctx = (o: Partial<ModuleLoadContext>): ModuleLoadContext => ({
  isAuthenticated: false,
  needsSetup: false,
  path: '/',
  permissions: [],
  platform: 'web',
  can: () => false,
  ...o,
})

const entry = (o: Partial<ModuleManifestEntry> & { name: string }): ModuleManifestEntry => ({
  routePaths: [],
  dependencies: [],
  load: async () => ({ default: {} as never }),
  ...o,
})

// ── isEligible ───────────────────────────────────────────────────────────────
test('isEligible: CORE entry (no shouldLoad) is always eligible', () => {
  assert.equal(isEligible(entry({ name: 'router' }), ctx({})), true)
  assert.equal(isEligible(entry({ name: 'router' }), ctx({ isAuthenticated: true })), true)
})

test('isEligible: auth-gated entry only when authenticated', () => {
  const e = entry({ name: 'chat', shouldLoad: c => c.isAuthenticated })
  assert.equal(isEligible(e, ctx({ isAuthenticated: false })), false)
  assert.equal(isEligible(e, ctx({ isAuthenticated: true })), true)
})

test('isEligible: permission-gated entry uses ctx.can', () => {
  const e = entry({
    name: 'user',
    shouldLoad: c => c.isAuthenticated && c.can('users::read'),
  })
  const nonAdmin = ctx({ isAuthenticated: true, can: () => false })
  const admin = ctx({ isAuthenticated: true, can: () => true })
  assert.equal(isEligible(e, nonAdmin), false, 'non-admin must NOT load the admin module')
  assert.equal(isEligible(e, admin), true, 'admin loads it')
})

test('isEligible: a throwing predicate is treated as not-eligible (never wedges)', () => {
  const e = entry({
    name: 'boom',
    shouldLoad: () => {
      throw new Error('boom')
    },
  })
  assert.equal(isEligible(e, ctx({})), false)
})

// ── coreEntries ──────────────────────────────────────────────────────────────
test('coreEntries returns only entries with no shouldLoad', () => {
  const m = [
    entry({ name: 'router' }),
    entry({ name: 'chat', shouldLoad: c => c.isAuthenticated }),
    entry({ name: 'auth' }),
  ]
  assert.deepEqual(coreEntries(m).map(e => e.name), ['router', 'auth'])
})

// ── orderByDependencies ──────────────────────────────────────────────────────
test('orderByDependencies places dependencies before dependents', () => {
  const m = [
    entry({ name: 'chat', dependencies: ['router'] }),
    entry({ name: 'router' }),
  ]
  const ordered = orderByDependencies(m).map(e => e.name)
  assert.ok(ordered.indexOf('router') < ordered.indexOf('chat'))
})

test('orderByDependencies tolerates a dependency outside the wave (subset load)', () => {
  // 'chat' depends on 'router' which is NOT in this wave — must not throw.
  const ordered = orderByDependencies([entry({ name: 'chat', dependencies: ['router'] })])
  assert.deepEqual(ordered.map(e => e.name), ['chat'])
})

test('orderByDependencies throws on a cycle', () => {
  const m = [
    entry({ name: 'a', dependencies: ['b'] }),
    entry({ name: 'b', dependencies: ['a'] }),
  ]
  assert.throws(() => orderByDependencies(m), /Circular/)
})

// ── entryForPath (route-driven loading) ──────────────────────────────────────
test('entryForPath matches static + param routes', () => {
  const m = [
    entry({ name: 'chat', routePaths: ['/chat', '/chat/:id'] }),
    entry({ name: 'settings', routePaths: ['/settings/users'] }),
  ]
  assert.equal(entryForPath(m, '/chat')?.name, 'chat')
  assert.equal(entryForPath(m, '/chat/abc123')?.name, 'chat')
  assert.equal(entryForPath(m, '/settings/users')?.name, 'settings')
  assert.equal(entryForPath(m, '/nope'), undefined)
})

// ── build-time extraction (the plugin) ───────────────────────────────────────
const MOD = (body: string) => `
import { createModule } from '@ziee/framework'
import { Permissions } from '@/api-client/permissions'
export default createModule({
  metadata: { name: 'demo', version: '1.0.0' },
  ${body}
})
`

test('extractModule pulls name, routePaths, dependencies', () => {
  const src = MOD(`
    dependencies: ['router'],
    routes: [{ path: '/demo', element: X }, { path: '/demo/:id', element: Y }],
  `)
  const ex = extractModule('demo/module.tsx', src)
  assert.equal(ex.name, 'demo')
  assert.deepEqual(ex.routePaths, ['/demo', '/demo/:id'])
  assert.deepEqual(ex.dependencies, ['router'])
  assert.equal(ex.shouldLoadSrc, null)
})

test('extractModule lifts a shouldLoad referencing ctx + Permissions', () => {
  const src = MOD(`shouldLoad: (ctx) => ctx.isAuthenticated && ctx.can(Permissions.UsersRead),`)
  const ex = extractModule('demo/module.tsx', src)
  assert.match(ex.shouldLoadSrc, /ctx\.can\(Permissions\.UsersRead\)/)
  assert.equal(ex.usesPermissions, true)
})

test('extractModule REJECTS a shouldLoad referencing a non-ctx/Permissions identifier', () => {
  const src = MOD(`shouldLoad: (ctx) => ctx.isAuthenticated && someImportedFlag,`)
  assert.throws(() => extractModule('demo/module.tsx', src), /someImportedFlag/)
})

test('extractModule allows ctx-only shouldLoad without Permissions', () => {
  const src = MOD(`shouldLoad: (ctx) => ctx.isAuthenticated,`)
  const ex = extractModule('demo/module.tsx', src)
  assert.equal(ex.usesPermissions, false)
})
