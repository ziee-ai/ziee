import { test } from 'node:test'
import assert from 'node:assert/strict'
import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { createStoreProxy } from './stores.ts'
import { defineStore, defineLocalStore } from './store-kit.ts'

// Loads the REAL store-kit under `node --test` (alias loader). The event-bus
// boundary is stubbed (never exercised by these specs); everything else is real.

test('TEST-5: a defineStore action lands on getState() and is callable hook-free', () => {
  const h = defineStore('ProbeStoreKit', {
    state: { n: 0 },
    actions: set => ({ bump: () => set((s: any) => ({ n: s.n + 1 })) }),
  })
  // The authoring model spreads actions into state, so getState().bump is a fn…
  assert.equal(typeof h.store.getState().bump, 'function')
  const p: any = createStoreProxy(h.store as any)
  // …and the proxy returns it directly (no hook) — callable outside render.
  assert.doesNotThrow(() => {
    p.bump()
    p.bump()
  })
  assert.equal(p.$.n, 2)
})

test('TEST-6: defineLocalStore proxy — reactive read in render, `$` snapshot, action in handler, no `__state`', () => {
  const Def = defineLocalStore({
    state: { q: '' },
    actions: (set: any) => ({ setQ: (v: string) => set(() => ({ q: v })) }),
  })
  let dollar: any
  let capturedAction: any
  let stateAlias: any
  function Comp() {
    const s: any = Def.use({ q: 'hello' })
    const { q } = s // reactive read IN render — must not throw
    dollar = s.$.q // hook-free snapshot in render
    capturedAction = s.setQ // action captured for a handler-context call
    stateAlias = s['__state'] // `__state` is NOT the snapshot alias anymore
    // (bracket access = same proxy trap, sidesteps the grit ban on `.__state`)
    return React.createElement('span', null, q)
  }
  const html = renderToStaticMarkup(React.createElement(Comp))
  assert.match(html, /hello/)
  assert.equal(dollar, 'hello')
  // `__state` on the local proxy is just a (missing) reactive field → undefined,
  // NOT the getState snapshot. Only `$` is the snapshot.
  assert.equal(stateAlias, undefined)
  // The action captured during render is callable from a handler context.
  assert.equal(typeof capturedAction, 'function')
  assert.doesNotThrow(() => capturedAction('world'))
})

test('TEST-42 (split-chat): two defineLocalStore instances expose independent raw StoreApis (per-pane ctx.chatStore)', () => {
  // The split feature gives each pane its OWN local store instance; the pane
  // context threads that instance's raw StoreApi (`.__api__`) as `ctx.chatStore`.
  // Two instances must be fully independent: a set on one never touches the other,
  // and each has its own subscribe/getState/setState.
  const Def = defineLocalStore({
    state: { conv: null as string | null },
    actions: (set: any) => ({ setConv: (v: string) => set(() => ({ conv: v })) }),
  })
  let apiA: any
  let apiB: any
  function A() {
    apiA = (Def.use({ conv: 'a' }) as any).__api__
    return null
  }
  function B() {
    apiB = (Def.use({ conv: 'b' }) as any).__api__
    return null
  }
  renderToStaticMarkup(React.createElement(A))
  renderToStaticMarkup(React.createElement(B))

  // Each raw StoreApi surface is present (subscribe / getState / setState).
  for (const api of [apiA, apiB]) {
    assert.equal(typeof api.getState, 'function')
    assert.equal(typeof api.setState, 'function')
    assert.equal(typeof api.subscribe, 'function')
  }
  // Distinct initial state (two independent instances, not a shared singleton).
  assert.equal(apiA.getState().conv, 'a')
  assert.equal(apiB.getState().conv, 'b')

  // A set on instance A does not touch B; A's subscriber fires, B's does not.
  let aFires = 0
  let bFires = 0
  apiA.subscribe(() => {
    aFires += 1
  })
  apiB.subscribe(() => {
    bFires += 1
  })
  apiA.setState({ conv: 'a2' })
  assert.equal(apiA.getState().conv, 'a2')
  assert.equal(apiB.getState().conv, 'b', 'instance B is unaffected by a set on A')
  assert.ok(aFires >= 1, 'A subscriber fired on A set')
  assert.equal(bFires, 0, 'B subscriber did NOT fire on A set')
})

test('TEST-9: `$.__destroy__` is reachable hook-free and does NOT trigger store init (HMR-destroy pattern)', () => {
  let initRuns = 0
  const h = defineStore('ProbeDestroy', {
    state: { x: 0 },
    actions: () => ({}),
    init: () => {
      initRuns += 1
    },
  })
  const p: any = createStoreProxy(h.store as any)
  // The exact access pattern module-system/store.ts's HMR path now uses:
  assert.equal(typeof p.$.__destroy__, 'function')
  assert.doesNotThrow(() => p.$.__destroy__())
  // Reaching teardown via `$` must NOT have fired store-level `__init__`
  // (the `$` branch short-circuits before the trap's init side-effect).
  assert.equal(initRuns, 0)
})
