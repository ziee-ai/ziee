# Canonical Store Pattern — unified lazy-by-default store registration

**Status:** DESIGN (co-developing). Graduates to a durable product-tree convention
(e.g. `agent-kit/docs/` + a lint) once finalized — it must NOT live only in
`.lifecycle/` (that's stripped at merge). Backs ITEM-3/ITEM-4 of frontend-perf.

**Goal:** ONE pattern every store follows — current and future — so store
placement (eager vs lazy chunk) is *automatic and emergent*, not a per-store
manual decision. The lazy-loading bundle win becomes a side effect of a clean,
uniform convention rather than a special mode with a taxonomy to maintain.

Companion: `LAZY_STORE_RESEARCH.md` (the option analysis + ecosystem prior art +
per-store safety sweep this design distills).

---

## 0. REFINED DIRECTION (supersedes the module-lazy framing below) — user, this session

The primary mechanism is **per-action lazy loading**, not whole-store lazy loading.
"Most of a store's code is in its actions" → split the actions out, one per file,
and load only the action being invoked.

**Pattern — folder per store:**
```
modules/users/store/
  Users.ts              // state (eager, tiny) + action MAP: name -> () => import('./actions/<name>')
  actions/
    loadUsers.ts        // ONE async action per file -> its own chunk; default export (set,get) => async (...) => {}
    createUser.ts
    deleteUser.ts
```
Rules (strict, so nothing is ambiguous to author or agent):
- **State is EAGER + always present** → `Stores.X.field` / `X.field` never `undefined`.
  This DISSOLVES the cross-module early-read hazard, so the module-lazy /
  self-register / retire-`Stores.X` machinery (§2–§7 below) is **NOT needed**;
  `Stores.X` stays and stays safe. Direct-handle imports remain optional (typing).
- **Every action is its own file and is ASYNC.** No sync "actions" through this
  path (trivial `set(...)` stays inline in the store). Enforced by the action-file
  signature + a lint. All-async removes the sync/async judgment call entirely.
- **Load-on-invoke:** `X.deleteUser(id)` loads only `deleteUser`'s chunk, then runs.
- **Preload on intent:** `X.deleteUser.preload()` on hover/focus warms the chunk so
  the click is instant (D3, now per-action).

**Store eager footprint** shrinks to: state shape + a small map of `import()`
thunks. All action weight (logic + ApiClient method imports) moves to per-action
chunks.

**Tradeoff to validate in the proof:** many small chunks (~90 stores × N actions).
Measure entry-chunk delta AND chunk-count/build overhead; fall back to
per-store-actions grouping if the count is unacceptable.

**Hard part to prove:** type safety — `X.deleteUser(id)` stays fully typed though
the impl is a dynamic import (infer the signature from `typeof import('./actions/deleteUser')`).

**Reconciliation with earlier D1–D4:** D1 (retire `Stores.X`) becomes OPTIONAL —
state-eager keeps `Stores.X` safe, so we keep it and drop the risky retire/codemod.
D2 proof-first stands (proof store: `ChatHistory` — an eager boot store, the
compelling case: state stays for the sidebar, actions go lazy). D3 preload → now
per-action, hover-triggered. D4 registry → unchanged.

---

## 0b. PROOF RESULT (WebSearchAdmin, measured) — the decisive finding

Built the per-action mechanism (`lazyActions` in `store-kit.ts`) + converted
`WebSearchAdmin` (4 actions → 4 files). Functional: PASS (page renders, `init`
fires the lazy loaders → their chunks load + execute, 0 errors, types intact).

**But the byte reality is the finding:** each action chunk is **~0.3 KB**
(`loadSettings` 0.29, `loadProviders` 0.24, `updateSettings` 0.31, `updateProvider`
0.33 — **1.17 KB raw across all 4**). Each chunk is `import{Ft}from"./index-…"` —
it pulls `ApiClient` from the **SHARED** chunk; it does NOT bundle it.

**Conclusion — "most of the code is in actions" is true by LINE COUNT, not by
BUNDLE WEIGHT.** An action's *unique* code is thin glue (~0.3 KB); the weight it
uses (`ApiClient`, which every store shares and which stays eager/shared
regardless) does not move when you split the action. So **blanket per-action
splitting produces hundreds of ~0.3 KB chunks for near-zero byte savings AND a
worse request waterfall** (a chunk + HTTP round-trip per action on use).

**Where per-action splitting DOES pay:** an action importing a **heavy, UNIQUE**
dependency (xlsx, a chart lib, a wasm module, a big client-side transform) that
would otherwise bloat the eager chunk. Rare, but the mechanism now serves exactly
those cases — use it SELECTIVELY, not as the default for thin API-call actions.

**Revised recommendation:** keep the `lazyActions` mechanism (built, tested,
additive, zero-risk to the other 90 stores) for the heavy-dep case. The bulk
frontend win stays at the layers already shipped (ITEM-1/2/5/9/10) + the module-
level split of admin stores (ITEM-3, ~43 KB gzip) — NOT per-action splitting of
thin stores.

## 1. The problem (verified mechanism)

Every one of ~90 stores is baked into the **482 KB gzip entry chunk**, not
because anything reads it at boot, but because of a static import chain:

1. `src/modules/loader.ts:81` — `import.meta.glob('./**/module.tsx',{eager:true})`
   pulls every `module.tsx` into the entry graph (intended — the shell needs
   routes/slots/permissions at boot).
2. Each `module.tsx` **statically imports its store file(s)** to pass them to
   `createModule({ stores: [...] })`.
3. The store file runs `defineStore(...)` → zustand `create()` at import
   (`store-kit.ts:196`), dragging in its whole transitive graph (ApiClient
   methods, `api-client/types`, permission helpers).

**The decisive fact:** the runtime is *already lazy* — a store's `__init__`
(SSE subscriptions + initial fetch) runs on FIRST property access
(`stores.ts:239`), and it's ref-count-destroyed 5 s after its last reader
unmounts (`stores.ts:130-197`). An eagerly-registered-but-unaccessed store never
subscribes and never fetches — it is **inert code** in the entry chunk. So making
it lazy is a pure bundling change with **zero runtime-semantics delta**.

**What tethers a store to a chunk:** the static `import` of the store file (step
2) and any barrel that statically re-exports it. A `Stores.X` *read* does NOT
tether — it's a runtime string lookup, not an import.

**The maintenance trap we are avoiding:** a per-store taxonomy ("Tier 1 / drawer-
only / chat-coupled / stays-eager") is unmaintainable — a feature author should
never have to classify their store. We want one rule that produces the right
placement automatically.

---

## 2. The unified rule

> **A store self-registers on import. A consumer imports the store it reads.**

Placement is then emergent, decided by the bundler:
- All of a store's readers live in lazy chunks → it lands in a shared lazy chunk. **Lazy.**
- Any reader lives in an eager chunk (boot/shell/eagerly-globbed) → it lands in the entry chunk. **Eager.**

No tiers. `Auth` is eager because `AuthGuard` (eager) imports it; `Users` is lazy
because only the lazy `UsersSettings` page imports it; `AssistantPicker` is eager
because the eagerly-globbed chat-extension imports it — **the same rule produced
all three**, nobody classified anything. A new admin store is lazy *by default*
(its only importer is its lazy page) and becomes eager only the day something
eager imports it.

### Dedup is free
ES modules evaluate **once**; a module imported by multiple chunks is hoisted by
Rollup/Rolldown into **one shared chunk**. So even a multi-reader store
(`Workflow` read by the workflow drawer + hub tab + scheduler picker) is
evaluated once, registered once — whichever reader loads first pulls in the shared
chunk. `registerStore` idempotent-by-name is a cheap backstop, not a necessity.

---

## 3. Authoring model (identical for every store)

**Define — always:**
```ts
// modules/users/Users.store.ts
export const Users = defineStore('Users', { state, actions, init })
//   ↑ defineStore CREATES the zustand store, wraps it in createStoreProxy ONCE,
//     self-registers it into the module-system registry, and RETURNS the proxy.
```

**Read — always, from anywhere (same-module or cross-module, lazy or eager):**
```ts
import { Users } from '@/modules/users/Users.store'  // import = register + typed handle
const { list, loading } = Users     // reactive read (same proxy ergonomics as today)
Users.$.list                        // snapshot read (handlers/async) — unchanged
Users.loadUsers()                   // action — unchanged
```

The import **is** the registration trigger, so placement falls out of the bundler.
No `stores: []` in `module.tsx`, no manual `registerStore` call sites, no tier to
pick. To read a store you must import it — which is exactly what makes the rule
self-enforcing.

---

## 4. Lifecycle guarantee (the load-bearing invariant)

**The store lifecycle — init-on-first-access, ref-counting, release/destroy after
last unmount, cache/re-init — lives ENTIRELY inside the per-store proxy built by
`createStoreProxy`, NOT in the global `Stores` registry.** Evidence
(`sdk/packages/framework/src/stores.ts`):
- The `refTracker` closure (ref counts, `scheduleDestroy`/`executeDestroy`,
  `destroyed`, `storeInitialized`) is created **once inside `createStoreProxy`**
  (`:~110-211`).
- The returned Proxy's `get` trap owns: init-on-first-access (`:239` runs
  `__init__.__store__()` = SSE subscribe + initial fetch), ref-count on reactive
  reads, and `__destroy__` on last unmount (`:130`→`:167` — clears SSE/event-bus
  listeners, resets init flags so it re-fetches on next access).
- The global `Stores` proxy (`:324`) is a one-line `stores[name]` lookup — it
  never participates in init/ref-count/release/cache.

**Consequence:** retiring `Stores.X` for direct-handle imports loses **zero**
lifecycle behavior, because the lifecycle is a property of the proxy *object*, not
of how you reach it. `import { Users }` returns that same lifecycle-owning proxy.

**The one invariant to preserve — exactly ONE proxy instance per store** (so the
ref-count is global across all readers). Guaranteed for free by ES module
singletons: `defineStore` runs once per module eval, creates one proxy, every
importer gets that one object. (Change from today: proxy is created at
*definition* instead of at *registration* — both are once; cleaner ownership, no
behavior change.)

---

## 5. The `Stores.X` global — decision

Today's `Stores.X` global proxy is what *breaks* uniformity: it lets you read a
store without importing it, which severs the register trigger and is the root of
the "who registers it" problem.

**Decision: retire `Stores.X` for direct-handle imports, with a non-breaking
compat shim during migration.**
- Reads become `import { X } from '…/X.store'; X.field` — one artifact does
  register + read + type.
- **Bonus:** fixes the VSCode `Stores.X is any` issue — a direct import resolves
  natively; no fragile `declare module` / `RegisteredStores` merging, no
  `import './types'` augmentation files needed for store typing.
- **Compat shim:** keep `Stores` working as a thin registry-backed proxy over the
  same per-store proxies during migration, so the ~137 existing `Stores.X` sites
  keep working (and keep their lifecycle) until a codemod migrates them. Delete
  the shim when the last site is converted.
- A `Stores`-style *dynamic enumeration* registry MAY remain for genuinely
  dynamic needs (devtools, "list all live stores") — but it is not the read path.

Rejected alternative: keep `Stores.X` + a lint requiring a paired register-import.
Uniform-with-a-caveat (two artifacts per read site + keeps the declaration-merge
machinery). Direct handles are the cleaner single pattern.

---

## 6. Guardrails (uniform, make regressions impossible)

1. **Dev-mode read-before-register warning** — the store proxy warns when a
   known-but-unregistered store is read, so an ordering mistake screams in dev
   instead of a silent prod `Cannot destructure undefined`.
2. **Lint** — "to read a store you must import it." Trivially satisfied by the
   direct-handle model (you can't read without importing); still worth a rule to
   ban re-introducing a decoupled global read.
3. **Budget fence (ITEM-8)** — entry-chunk gzip + login-chunk-count check catches
   any store drifting back into the entry chunk (e.g. a future eager reader).
4. **Type safety** — native via direct import (no declaration merging to keep in
   sync). During compat, `RegisteredStores` stays for the shim.
5. **Desktop parity** — `loader.desktop.ts` + `CORE_MODULE_BLOCKLIST`: a blocked
   module's page never loads → its store never registers (consistent); verify no
   eager path imports a blocked module's store file.

---

## 7. Migration plan (non-breaking, proof-first)

1. **Mechanism:** `defineStore` creates+owns+exports the proxy and self-registers;
   add a `registerStore` action on `useModuleSystemStore` (mirror the existing
   `newStores[name]=createStoreProxy(store)` at `store.ts:123`); keep `Stores` as
   a compat shim over the registry.
2. **Barrels (ITEM-4), per store touched:** remove the static barrel re-exports
   (`stores/index.ts`, framework/api-client barrels) that would re-tether a store
   to the entry graph (the `INEFFECTIVE_DYNAMIC_IMPORT` warnings in BASELINE B4).
3. **Proof (scoped):** convert ONE module (`user`) end-to-end to the direct-handle
   pattern; keep the shim so `Stores.User*` sites still work + keep lifecycle;
   verify pages render, types resolve in-editor, ref-count destroy still fires
   (watch the dev logs), and **measure** the entry-chunk delta.
4. **Codemod + sweep:** if the pattern + number hold, codemod the ~137 `Stores.X`
   sites and convert the rest module-by-module. Delete the shim at the end.

---

## 8. DECISIONS (resolved with the user)

- **D1 — Read API: KEEP `Stores.X` (REVERSED — do NOT retire).** Rationale
  (user): the type augmentation (`chat/types.ts` etc.) is written entirely with
  `import type` / `typeof` → **fully erased at build**, so `Stores.X` typing costs
  ZERO runtime + ZERO bundle. And with per-action lazy + EAGER STATE, `Stores.X.field`
  is never `undefined`, so there's no read-before-register hazard. Types are
  "lazy" (erased, free); code is lazy (per-action / module). The two are orthogonal.
  → **Drop the retire-`Stores.X` + 137-site codemod plan** — it was unnecessary
  churn. The VSCode `any` it would have "fixed" is a workspace-TS-version setting,
  not a real hole (tsc types it correctly). Direct-handle imports remain allowed
  but optional; `Stores.X` stays the primary read API.
- **D5 — Small chunks as anti-peek (user):** many tiny hashed chunks fragment the
  code + lazy per-action loading means a viewer only ever receives the action code
  they actually invoke (the full action surface is never shipped wholesale). This
  is a real reduced-exposure/obfuscation benefit (NOT security — chunks are still
  fetchable + reassemblable). It partially offsets the many-tiny-chunks cost and is
  a legitimate reason to favor broader per-action splitting despite the ~0 byte win.
- **D2 — Rollout: PROOF-FIRST.** Build the mechanism + convert ONE module (`user`)
  end-to-end, keep the shim, MEASURE the entry-chunk delta and confirm
  lifecycle/types/pages hold. Decide the full sweep after a real number.
- **D3 — PRELOAD: yes, add `preload()`.** A lazy store exposes `preload()` to warm
  its chunk + kick off `init` (fetch) on hover / route-intent, so page-open is
  instant instead of a fetch-on-open flash. Mirrors the route-chunk prefetch.
- **D4 — Enumeration registry: KEEP a read-only registry.** For devtools / "list
  live stores" / debugging — NOT the read path. The compat `Stores` shim naturally
  doubles as this; after migration it survives as a read-only enumeration surface.

Still-open (minor, resolve during build): Q2 store→store `dependsOn` (default:
implicit via import-to-register); Q4 persist/cross-tab interaction (verify a
persisted-but-not-yet-loaded store on a fresh tab behaves).

### Concrete API shape (from D1/D3/D4)
```ts
// defineStore now: create store → createStoreProxy ONCE → self-register → return proxy.
// The returned handle carries the SAME lifecycle proxy + a preload().
export const Users = defineStore('Users', { state, actions, init })
Users.preload()          // D3: warm chunk + run init early (idempotent, no-op if already inited)

// D4: read-only enumeration (devtools only, NOT the read path)
listRegisteredStores()   // → [{ name, refCount, initialized }]

// D1 compat shim (deleted post-migration):
Stores.Users             // returns the SAME proxy instance as `import { Users }`
```
