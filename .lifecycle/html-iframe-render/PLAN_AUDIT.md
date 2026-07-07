# PLAN_AUDIT — audit of PLAN.md against the codebase

Audited before writing code. Verdicts per ITEM at the bottom.

## Breakage risk

- **`plugins` prop on `<Streamdown>`** — new prop. Confirmed `StreamdownProps`
  includes `plugins?: PluginConfig` (`node_modules/streamdown/dist/index.d.ts`).
  Passing only `{ renderers: [...] }` does NOT clobber the default Shiki `code`
  highlighter: the custom-renderer dispatch is a language-keyed early-return
  (`t.renderers.find(l==='html')`), and for every non-`html` fence it falls
  through to the built-in `CodeBlock` (driven by the `shikiTheme` prop, which is
  unchanged). No `plugins.code` override is set, so built-in Shiki is preserved.
  Verified in `chunk-BO2N2NFS.js`: `if(u){...return jsx(component,...)}` then
  `if(m==="mermaid"&&d){...}` then default code block.
- **Two `<Streamdown>` call sites** (`components/TextContent.tsx` +
  `extensions/text/components/TextContent.tsx`) — both must receive `plugins` or
  the feature silently works in only one render path. The extension text renderer
  wins via `ContentRenderer.renderContent`, but `components/TextContent` is the
  built-in fallback; ITEM-6 edits BOTH. Both already share
  `useStreamdownComponents` + `streamdownUrlTransform`, so adding a shared
  `streamdownPlugins` const beside them is the established idiom.
- **Fenced ```html vs raw HTML-in-markdown** — orthogonal. The existing test
  `markdown-rendering.spec.ts::raw <script> tags in markdown do not execute`
  pins that Streamdown escapes raw HTML (no `rehype-raw`). This feature only
  touches FENCED `html` code blocks (the fence body arrives as an opaque `code`
  string to the custom renderer), so raw-HTML sanitization is untouched — no
  regression to that guard. New behavior is strictly additive.
- **Legacy `syntax` extension** (`extensions/syntax/extension.tsx`) parses code
  blocks with its own regex but is NOT on the live render path (the `text`
  extension + `components/TextContent` are). Left untouched; no collision.
- **Streaming** — the reducer feeds partial text; `isIncomplete` is supplied by
  Streamdown per-block. ITEM-4 gates render on it, so a half-open fence can't
  render. No new stream-path code; no reducer change.

## Pattern conformance

- `HtmlBlock` mirrors the copy/lucide/`message` pattern of the syntax
  `CodeBlock`; the toggle uses the kit `Segmented` (root + per-option testids,
  `data-state`); tokens are semantic per `DESIGN_SYSTEM.md`. Conforms.
- The gallery edits mirror `renderingShowcase` (fixture block list) and the
  `deep-chat-long` `interactions[]` recipe shape exactly. Conforms.
- The affordance rule mirrors the `RULES[]` object shape; guarding (no allowlist
  entry) is the documented "feature ships → rule guards" convention (§6/§7).
  Conforms.
- E2E mirrors `markdown-rendering.spec.ts` seeding + assertion style. Conforms.

## Migration collisions

None. Frontend-only feature: no SQL migration, no `migrations/` file, no DB.
`ls src-app/server/migrations | tail` is irrelevant here.

## OpenAPI regen

Not required. No Rust type, route, or response-shape change; no new/edited
`#[derive(ToSchema)]`. `src-app/desktop/ui` has no chat module (verified: no
`modules/chat` there), so no desktop client regen. The `openapi.json` /
`api-client/types.ts` are untouched, so the validator's generated-file excludes
are moot. `npm run check (ui)` still runs (tsc + biome guardrails + lint:colors +
check:kit-manifest + check:testid-registry + check:gallery-coverage +
check:state-matrix) — the design/testid/gallery gates DO apply and are covered by
Phase 8.

## New-risk callouts (not blockers, tracked into DECISIONS/audit)

- **`check:testid-registry`** — the two new testids (`html-block`,
  `html-block-toggle`, `html-block-copy-btn`) may need registry/manifest
  registration; resolved in DECISIONS (DEC-8).
- **`check:gallery-coverage` / `check:state-matrix`** — adding a fixture block +
  an interaction recipe must keep the generated coverage/state-matrix in sync;
  resolved in DECISIONS (DEC-9).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — Streamdown `plugins.renderers` seam confirmed in dist; `Segmented` kit supplies the toggle with the needed testids; default-Code via `defaultValue`.
- **ITEM-2** — verdict: PASS — pure module, unit-of-truth for the sandbox token + CSP; React `srcDoc` escaping means no manual HTML-string injection. No codebase collision.
- **ITEM-3** — verdict: PASS — standard `<iframe srcDoc sandbox>`; `sandbox="allow-scripts"` WITHOUT `allow-same-origin` is the documented safe combo; no existing iframe renderer to conflict with.
- **ITEM-4** — verdict: PASS — `isIncomplete` is a first-class custom-renderer prop (verified in dist: `jsx(component,{code,isIncomplete,...})`); gating is local component logic.
- **ITEM-5** — verdict: PASS — mirrors the syntax `CodeBlock` copy/label pattern; kit `Button` + `message` already imported across chat.
- **ITEM-6** — verdict: CONCERN — must edit BOTH `TextContent.tsx` files or the render path diverges; low-risk but easy to half-do. Mitigation: shared `streamdownPlugins` const + an e2e that exercises the live assistant path (which routes through the extension renderer), plus explicit Phase-6 patterns-conformance angle on both files.
- **ITEM-7** — verdict: CONCERN — gallery fixture/interaction edits can desync `check:gallery-coverage`/`check:state-matrix` generated snapshots; resolved by regenerating them in Phase 8 (DEC-9). Not blocking.
- **ITEM-8** — verdict: PASS — additive detector RULE + doc edits; no allowlist churn beyond adding a guarding rule; mirrors existing rules.
