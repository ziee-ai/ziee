# Gallery mock-API SEED system — cassette survey

Scope: `src-app/ui/src/dev/gallery/` fixtures + the `mockApi` replay layer +
the record/check/gen scripts. Goal: understand today's seed model to inform a
per-module ownership redesign.

---

## 1. How `GALLERY_CASSETTE` is assembled (order / override semantics)

Single central assembly in `src/dev/gallery/fixtures/index.ts`. It spreads 7
per-module cassettes into ONE flat object, in this order:

```ts
export const GALLERY_CASSETTE: Cassette = {
  ...crawlCassette,        // broad, recorded, FIRST (lowest priority)
  ...authCassette,
  ...llmProvidersCassette,
  ...chatCassette,
  ...citationsCassette,
  ...projectDeepCassette,
  ...workflowCassette,     // hand-authored, LAST (highest priority)
}
```

Override semantics = **plain object-spread last-wins on a per-endpoint-key
basis**. Comment in the file states the intent explicitly: "Broad crawl first;
hand-authored per-module fixtures LAST so they win (they carry richer,
purpose-seeded data + query/path-param resolvers)."

Concrete override examples:
- `crawlCassette` has a literal `Conversation.list`; `chatCassette` overrides it
  with the richer recorded conversation list.
- `crawlCassette` has `UserGroup.list`; `llmProvidersCassette` overrides it.
- `crawlCassette` has a literal `Citations.list`; `citationsCassette` overrides
  it with a `query.project_id`-keyed resolver.
- `LlmModel.list` is **deliberately DROPped from the crawl** (in
  `gen-crawl-cassette.mjs`'s `DROP` set) so the hand fixture's query-resolver is
  the sole provider — belt-and-suspenders beyond the spread order.

The cassette is consumed by `mockApi.ts`: `installMockApi(GALLERY_CASSETTE)`
replaces `window.fetch`, matches `/api/*` paths back to their endpoint key via
precompiled route regexes (`matchRoute`, most-specific-wins on fewest path
params), and answers from the cassette entry. Unmatched route OR unseeded key →
`makeSafeEmpty()` recursive array-like proxy (never crashes a consumer).

`index.ts` also re-exports non-cassette seed identities: `adminUser`,
`adminMe`, `adminPermissions` (from auth), `showcaseConversationIds` (from chat).

---

## 2. The `crawlCassette` — what it is / how broad / how generated

**What:** the broad, mechanically-recorded baseline. Every SAFE **paramless
GET** endpoint's live response, so list/settings pages across ALL modules
populate with SOMETHING without any hand authoring.

**Generation is a 2-stage pipeline:**
1. **Record** (`scripts/record-gallery-fixtures.mjs`, `crawl` recorder) →
   `fixtures/recorded/crawl.json` (raw `{ endpointKey: responseJSON }`,
   null-stripped). Selection filter: `method === 'GET'` AND path has no `{param}`
   AND not in `CRAWL_SKIP` (SSE/`/stream`, `/export`, `/download`, built-in
   `…/mcp$` JSON-RPC, `/local-llm/v1`, `/setup/status`, `/health`, `/auth/me`).
   Each is fetched with `?page=1&per_page=100` appended (harmless on
   non-paginated).
2. **Codegen** (`scripts/gen-crawl-cassette.mjs`, npm `gen:gallery-crawl`) →
   `fixtures/crawl.generated.ts` (marked AUTO-GENERATED / DO-NOT-EDIT). Reads
   `crawl.json`, drops `DROP` keys (`LlmModel.list`), splits the rest into a
   **typed block** (`satisfies Cassette` → each response tsc-checked against its
   `GetResponseType<K>`) and a **loose block** (6 keys in `LOOSE`:
   `Hub.getAssistants`, `Hub.getCatalog`, `Hub.getCatalogVersion`,
   `Hub.getModels`, `McpServer.listAccessible`, `McpServerSystem.list` —
   openapi-valid but stricter-than-types.ts, so cast `as unknown as Cassette`).
   `crawlCassette = { ...typed, ...loose }`.

**How broad / subset?** Current numbers: `crawl.json` = **61** recorded keys →
`crawl.generated.ts` emits **60** (`LlmModel.list` dropped) = **54 typed + 6
loose**. This is a **SUBSET, not all endpoints**:
- ONLY paramless GETs — every path-param detail endpoint (`Conversation.get`,
  `Project.get`, `LlmProvider.getGroups`, `Message.getHistory`, …) is excluded
  by construction and must come from a hand fixture (or fall to safe-empty).
- Mutations (POST/PUT/DELETE) are never crawled — `mockApi` passes them through
  as no-ops against loaded data.
- The `CRAWL_SKIP` classes above are excluded.
- Whatever the recording box returned as EMPTY stays empty (e.g. `Citations.list`
  recorded `{entries:[]}` → the reason `citations.ts` exists to override it).

**gate:** `check:gallery-crawl` = `gen-crawl-cassette.mjs --check` → fails if
`crawl.generated.ts` is stale vs `crawl.json` (must regen+commit after
re-recording).

---

## 3. The shape of a per-module fixture

A fixture is exactly `{ [endpointKey]: response | resolver }` — a partial map
over endpoint keys. Exact TS type (from `mockApi.ts`):

```ts
export interface MockRequestContext {
  params: Record<string, string>   // path captures, e.g. { provider_id }
  query:  Record<string, string>   // parsed querystring, e.g. { providerId, page }
  body:   unknown                  // parsed JSON body (mutations)
  method: string
}

export type CassetteEntry<K extends ApiEndpoint> =
  | GetResponseType<K>                              // literal recorded response
  | ((ctx: MockRequestContext) => GetResponseType<K>) // derive from request

export type Cassette = { [K in ApiEndpoint]?: CassetteEntry<K> }
```

So each value is EITHER a literal of the endpoint's response type OR a resolver
returning that type. Both are typed by `K`, so a wrong shape fails `tsc`.

Three concrete forms:

- **Literal** (`workflow.ts`):
  ```ts
  export const workflowCassette: Cassette = {
    'Workflow.dryRun': dryRun,   // DryRunResult literal
    'Workflow.test':   testRun,  // TestRunResponse literal
  }
  ```
- **Query-keyed resolver** (`llm-providers.ts`):
  ```ts
  'LlmModel.list': ({ query }) =>
    llmModelsByProvider[query.providerId] ?? emptyModels,
  ```
  (`citations.ts` similarly branches on `query.project_id` to return the
  populated library vs an empty project-scoped list.)
- **Path-param resolver** (`chat.ts` / `project-deep.ts`):
  ```ts
  'Conversation.get': ({ params }) =>
    (chatById[params.id] ?? chatById[firstId])?.conversation,
  'Project.get': ({ params }) =>
    params.id === DEEP_PROJECT_ID ? deepProject : { ...deepProject, id: params.id },
  ```

---

## 4. Which modules have HAND-AUTHORED fixtures vs crawl-only vs nothing

**Fixture file → module/endpoints it seeds (all merged centrally in index.ts):**

| File | Backing | Endpoint keys it OWNS in the cassette |
|---|---|---|
| `crawl.generated.ts` (`crawlCassette`) | recorded `crawl.json` | 60 paramless GETs across ALL modules (Assistant, Auth, Chat.getUserLlmProviders, Citations.list, CodeSandbox, Conversation.list, File.list, FileRagAdmin, Hardware, Hub.*, LitSearch.*, LlmProvider.*, LlmRepository, LocalRuntime.*, Mcp*, Memory*, Onboarding, Project.list, RuntimeVersion.*, ServerUpdate, Skill.list, SkillSystem.list, Summarization, User.list, UserGroup.list, WebSearch.*, Workflow.list/listSystem) |
| `auth.ts` (`authCassette`) | recorded `auth.json` + literal | `Auth.me`, `Auth.getSessionSettings` (the latter a hand literal — crawl never recorded it) |
| `llm-providers.ts` (`llmProvidersCassette`) | recorded `llm-providers.json` | `LlmProvider.list`, `LlmModel.list` (query resolver), `UserGroup.list`, `LlmProvider.getGroups` (param resolver) |
| `chat.ts` (`chatCassette`) | recorded `chat.json` + `chat-deep.ts` | `Conversation.list`, `Conversation.get`, `Message.getHistory`, `Message.searchInConversation`, `Branch.list` (all param/query resolvers) |
| `citations.ts` (`citationsCassette`) | hand literal | `Citations.list` (query resolver) |
| `project-deep.ts` (`projectDeepCassette`) | hand literal | `Project.get`, `Project.listFiles`, `Project.listConversations` |
| `workflow.ts` (`workflowCassette`) | hand literal | `Workflow.dryRun`, `Workflow.test` |

**Hand-authored (override the crawl / add un-crawlable endpoints):** auth,
llm-providers, chat (+chat-deep), citations, project-deep, workflow.

**Crawl-ONLY (no hand fixture; populated solely by the broad crawl):** the
long tail — Assistant/AssistantTemplate, CodeSandbox, File.list, FileRagAdmin,
Hardware, Hub, LitSearch, LocalRuntime, Mcp/McpToolCall/McpUserPolicy,
Memory/MemoryAdmin/MemorySettings, Onboarding, RuntimeVersion, ServerUpdate,
Skill.list, SummarizationAdmin, User.list, WebSearch, and the LIST forms of
Project/Workflow/Conversation.

**NOTHING (fall to `makeSafeEmpty()`):** any endpoint not crawled AND not
hand-seeded — chiefly path-param detail endpoints of crawl-only modules, and any
POST/PUT/DELETE. `mockApi` logs a dev `console.warn` ("no cassette for …" / "no
route for …") and returns the safe-empty proxy. Several hand fixtures exist
precisely to plug a safe-empty crash (`project-deep.ts` `Project.listFiles`;
`workflow.ts` dialogs; `citations.ts` `empty`-mode distinction).

**IMPORTANT — non-cassette seeds (NOT in `GALLERY_CASSETTE`):** two fixture
files feed the gallery through a DIFFERENT channel (direct store-seeding via
`holdPatch`, SSE replay, `deepStates.tsx`/`overlays.tsx`), NOT the cassette:
- `skills.ts` — imported ONLY by `overlays.tsx` (seeds `Stores.Skill` /
  `Stores.ConversationSkills` directly). It exports raw arrays, no `Cassette`.
- `chat-deep.ts` — its `chatDeepById` bundles ARE merged into `chatCassette`
  (via `chat.ts`), but its transient seeds (`streamingCassette` SSE frames,
  `liveElicitation`/`liveAskUser`, `rightPanelFile`, `literaturePanelData`) are
  driven through the real store by `deepStates.tsx`.
- `project-deep.ts` likewise exports `deepProjectConversations` /
  `deepProjectFiles` that are `holdPatch`ed onto stores by the deep-surface, on
  top of its thin cassette entries.

So "seed" today has TWO mechanisms: (a) the fetch-replay **cassette**, and
(b) direct **store-seeding** for transient/live/deep states that a static GET
body can't express.

---

## 5. How endpoint keys are typed (build-time safety)

From `@/api-client/types` (generated from openapi by the Rust `emit_ts.rs`):
- `ApiEndpoints` — a `… as const` object `{ 'Module.method': 'GET /api/path/{p}' }`.
  `mockApi.ts` compiles each into a route regex; `record`/`check`/`gen` scripts
  regex-parse the same block.
- `ApiEndpoint` — the union of those keys (`keyof typeof ApiEndpoints`).
- `GetResponseType<K>` — maps a key to its response body type.

`Cassette = { [K in ApiEndpoint]?: CassetteEntry<K> }` and
`CassetteEntry<K> = GetResponseType<K> | (ctx) => GetResponseType<K>` mean every
cassette value is checked against the REAL API response type for that key. This
is **layer-1** of a 3-layer correctness contract stated in the file headers:
1. **tsc** — typed against `GetResponseType<K>` (the typed crawl block +
   every hand `fixtures/*.ts`); drift fails the build.
2. **record** — bodies are recorded from a real server, so they're correct by
   construction (`record-gallery-fixtures.mjs`).
3. **ajv contract** — `check-gallery-fixtures.mjs` validates recorded bodies
   against `openapi.json` component schemas.

The 6 `LOOSE` crawl keys are the one gap in layer-1 (cast, not `satisfies`);
they rely on layer-3 (ajv) instead.

---

## 6. The recording flow (`gallery:record`) + `check-gallery-fixtures`

**`npm run gallery:record` = `record-gallery-fixtures.mjs`:**
- Needs a REAL ziee server. Boots one against a throwaway embedded-Postgres +
  temp data dir (`/data/pbya/ziee/tmp/gallery-record-*`), writes an ephemeral
  `record.yaml` (embedded PG, sandbox off, known JWT secret). Uses `ZIEE_BINARY`
  if set, else `cargo build -p ziee`.
- Runs first-run setup with a fixed admin, logs in for a token, then
  `psql`-loads `server/seeds/showcase/showcase.sql` (best-effort — provides the
  multi-state chat conversations owned by that admin).
- Recorders: `auth` (`/auth/me`), `chat` (list + per-conversation
  detail/messages/branches → `chat.json`), `crawl` (all safe paramless GETs →
  `crawl.json`), `llm-providers` (ACTIVELY mutates: enables anthropic/openai/
  gemini/deepseek via the real PUT + creates models via the real POST, then reads
  back `providers`/`modelsByProvider`/`groups`/`groupsByProvider`). Crawl runs
  LAST. All bodies `stripNulls`'d and written to `fixtures/recorded/*.json`.
  `--only=` scopes recorders (auth always included).
- **This IS how a new module gets seed data**, but with big caveats: it only
  auto-covers **paramless GETs** (they flow into the crawl for free). Any
  detail/path-param endpoint, any populated-state that a fresh DB won't produce,
  or any query-keyed shape requires EITHER a bespoke recorder arm added to this
  script (like `chat`/`llm-providers`) OR a hand `fixtures/*.ts`. After recording,
  you must re-run `gen:gallery-crawl` and wire any new hand file into `index.ts`.

**`npm run gallery:check-fixtures` = `check-gallery-fixtures.mjs` (ajv, exit 1
on fatal):**
- Asserts recorded cassette JSON matches `openapi/openapi.json` response schemas.
- A hardcoded `MANIFEST` maps specific recorded sub-objects to component schemas
  (FATAL on mismatch): `auth.json:me → MeResponse`;
  `llm-providers.json` `providers → LlmProviderListResponse`,
  `modelsByProvider.* → LlmModelListResponse`, `groups → GroupListResponse`.
- Then AUTO-validates **every** `crawl.json` key against its endpoint's openapi
  200/201 JSON schema (resolves key → METHOD+path via the parsed `ApiEndpoints`,
  then to the schema `$ref`). Crawl drift is **reported as a WARNING, non-fatal**
  (crawl may be recorded from an older reference binary + falls back to
  safe-empty); the MANIFEST hand fixtures are the only FATAL checks.
- It does NOT assert cassette-vs-openapi COMPLETENESS (no "every endpoint must be
  seeded" gate) — only that whatever IS recorded is schema-valid. `chat.json`,
  `citations`/`project-deep`/`workflow` (which are code-literals, not recorded
  JSON) are not in the ajv manifest; their contract is tsc (layer-1) only.

---

## 7. Per-module ownership today?

**No true per-module ownership. It is centralized.** The only assembly point is
`fixtures/index.ts`, which hardcodes the import + spread of all 7 cassettes and
their precedence order. Adding a module's seed = create `fixtures/<mod>.ts` AND
edit `index.ts` (import + add to the spread in the right position). The crawl is
one shared generated file; the recorder is one shared script with per-module
recorder arms baked in. There is no registry, no auto-discovery, no
`module → owns these endpoint keys` manifest, and no per-module co-location with
the actual UI module (`src/modules/<x>/`). The header comment "Add a module here
as its fixture lands" confirms the centralized, manual model.

Secondary centralization: the two non-cassette seed channels are also central —
`overlays.tsx` hardwires the `skills.ts` import; `deepStates.tsx` hardwires the
`chat-deep.ts` transient seeds. A per-module redesign would need to cover BOTH
the cassette map AND these store-seed hooks.

### Redesign-relevant seams
- `Cassette` is already a clean per-key partial map → merging N per-module
  cassettes is trivial; the only real decision is **precedence** (today: crawl
  first, hand last; a module owning its own keys removes most override needs).
- The crawl is the "free baseline"; a per-module model could keep the crawl as a
  fallback layer and let each module OWN + override its keys.
- `DROP`/`LOOSE` sets in `gen-crawl-cassette.mjs` and the `MANIFEST` in
  `check-gallery-fixtures.mjs` are additional CENTRAL lists a module currently
  can't extend without editing shared files.
