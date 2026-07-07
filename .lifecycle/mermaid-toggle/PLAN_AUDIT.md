# PLAN_AUDIT — Mermaid code⇄render toggle

Audit of PLAN.md against the actual codebase (Streamdown 2.5.0 dist inspection,
the chat render paths, the kit, the gallery/detector infra) before writing code.

## Breakage risk

- **Existing mermaid rendering is replaced, not extended.** Today Streamdown's
  built-in `code` component renders mermaid with its own `mermaid-block-actions`
  (download/copy/fullscreen, gated by `controls.mermaid`). Registering a
  `plugins.renderers` entry for `mermaid` makes `ao('mermaid')` resolve OUR
  component FIRST (verified in `dist/chunk-BO2N2NFS.js`: `ao=e=>{...t.renderers.find
  ...}` is checked before `if(m==="mermaid"&&d)`), so the built-in toolbar is
  bypassed. No visual double-toolbar. Non-mermaid code fences are untouched (the
  custom renderer only claims `language: 'mermaid'`).
- **Two live TextContent render paths.** `extensions/text/components/TextContent.tsx`
  (registered via the text extension) AND `components/TextContent.tsx` (used by
  `components/ContentRenderer.tsx`) both mount `<Streamdown>`. Both must receive the
  `plugins` prop or the toggle would appear on only one path. PLAN wires both.
- **StreamdownErrorBoundary blast radius.** Both `<Streamdown>` calls are wrapped in
  `StreamdownErrorBoundary`; an uncaught throw in our renderer would blank the whole
  message. Mitigated: ITEM-3 catches render errors internally (try/catch around the
  async `mermaid.render`) and renders an error state instead of throwing.
- **Streaming thrash.** A mermaid fence updates char-by-char while streaming; naive
  rendering would spam mermaid parse errors and churn. Mitigated: ITEM-3 defers
  rendering while `isIncomplete` (Streamdown passes the per-fence incomplete flag),
  and a cancellation guard discards stale async SVG results.
- **Bundle size.** `mermaid` is heavy (~pulls d3). Static import would bloat every
  chat load. Mitigated: dynamic `import('mermaid')` inside the render effect
  (mirrors Streamdown's own lazy mermaid chunk), so it loads only when a mermaid
  block first renders.
- **Detector container invariant.** The `code-copy` detector rule asserts every
  `[data-streamdown="code-block"]` contains a copy button. PLAN renders the SOURCE
  view as a plain `<pre>` (NOT Streamdown's `CodeBlock`, which stamps that marker),
  so no bare code-block-without-copy is emitted; copy-source lives in the toolbar.

## Pattern conformance

- **ITEM-1** mirrors `MarkdownTable.tsx` one-to-one (same card chrome, header bar,
  `data-streamdown` marker, ghost-button toolbar, `data-testid` convention). This is
  the closest existing module and the required project pattern
  ([[feedback_match_existing_patterns]]).
- **ITEM-2** uses the kit `Segmented` exactly as designed (required `data-testid`,
  `options`, controlled `value`/`onValueChange`).
- **ITEM-4/5** use kit `Button` under the Spec-B variant policy (peer icon buttons →
  one `ghost` variant, mandatory tooltip on `size="icon"`), the `message` toast
  idiom, and the export-extension Blob-download idiom.
- **ITEM-6** mirrors the shared-hook pattern of `useStreamdownComponents.tsx` (one
  shared config imported by both TextContent files) rather than duplicating inline.
- **ITEM-7** follows the `GalleryStory`/`cases` story contract + `stories/index.ts`
  registration.

## Migration collisions

- None. No SQL migration, no DB table, no permission. `ls migrations/` is irrelevant
  to this branch — it is a pure frontend change.

## OpenAPI regen

- None required. No backend types, no route, no response shape change → no
  `openapi.json` / `api-client/types.ts` regen in either the server or desktop
  binary. The `types_ts_parity` golden test is unaffected. (Confirmed: no file under
  `server/` or any `*.rs` is in "Files to touch".)

## Per-item verdicts

- **ITEM-1** — verdict: PASS — direct structural mirror of `MarkdownTable.tsx`; new file, no caller breakage.
- **ITEM-2** — verdict: PASS — kit `Segmented` used per its documented API; `data-testid="mermaid-source-toggle"` satisfies the detector contract already encoded in `affordance-audit.mjs`.
- **ITEM-3** — verdict: CONCERN — mermaid render is async + heavy; requires lazy-import, an `isIncomplete` defer, an internal try/catch (so StreamdownErrorBoundary is not tripped), and a stale-result cancellation guard. All four are specified in the item; noted here as the highest-care area.
- **ITEM-4** — verdict: PASS — clipboard+toast is the established `MessageActions` idiom.
- **ITEM-5** — verdict: PASS — Blob download is the established export-extension idiom; disabled-until-rendered avoids a null-SVG download.
- **ITEM-6** — verdict: CONCERN — MUST wire BOTH TextContent paths or coverage is partial; `plugins.renderers` is a public Streamdown API (typed `PluginConfig`), verified present in the 2.5.0 dist. Shared-config module keeps the two call sites in lockstep.
- **ITEM-7** — verdict: PASS — additive gallery story; `MermaidBlock` is self-contained (no StreamdownContext needed for source `<pre>` or direct `mermaid` render) so it renders standalone in a gallery case.
- **ITEM-8** — verdict: PASS — allowlist removal is the documented "feature ships → detector guards" step (`affordance-audit-allowlist.json` `_comment`); the rule + selector already exist in `affordance-audit.mjs`.
