# PLAN — HTML code-block sandboxed-iframe render (code⇄render toggle)

Feature slug: `html-iframe-render` · Branch: `feat/html-iframe-render` ·
Worktree: `/data/pbya/ziee/tmp/w2b-wt`

## Context

A fenced ```` ```html ```` code block in an assistant chat message today renders
as an inert, Shiki-highlighted code block — the raw HTML is never live-rendered
(`docs/AFFORDANCE_MATRIX.md` gap **G3**, backlog **§7(b)**). This feature gives
each `html` block a **Code | Preview** toggle. **Code** (the DEFAULT, for safety)
shows the highlighted source; **Preview** renders the HTML inside a **strictly
sandboxed iframe** (`sandbox="allow-scripts"` with **NO** `allow-same-origin`,
`srcdoc`, an injected `<meta>` CSP that blocks all external network, no
top-navigation, no popups, no forms). The user opts into render **per block**.

Frontend-only (`src-app/ui`). Desktop (`src-app/desktop/ui`) has **no** chat
module, so no desktop mirror and no OpenAPI/type regen. Streamdown 2.5.0 exposes
a first-class `plugins.renderers` custom-renderer hook (the same mechanism its
mermaid plugin uses) — `{ component, language }`; the component receives
`{ code, isIncomplete, language, meta }`. That is the clean seam; no fork, no
override of Streamdown's `code`/`pre` element.

## Items

- **ITEM-1**: New shared component `HtmlBlock` (the Streamdown custom-renderer for `language: 'html'`) with a `Code | Preview` `Segmented` toggle whose selection defaults to **Code**; renders the code body or the preview body per the selected mode; container carries `data-testid="html-block"`, toggle carries `data-testid="html-block-toggle"`.
- **ITEM-2**: Pure, side-effect-free sandbox helper module `htmlBlockSandbox.ts` exporting the `sandbox` token string (`allow-scripts` only — no `allow-same-origin`/`allow-top-navigation`/`allow-popups`/`allow-forms`/`allow-modals`), the conservative CSP string (blocks all external network), and `buildSandboxedSrcdoc(html)` that injects the CSP `<meta>` into the document `<head>` so the browser enforces it before any user markup runs.
- **ITEM-3**: Preview body renders an `<iframe>` fed by `srcDoc={buildSandboxedSrcdoc(code)}` with the ITEM-2 `sandbox` token, `referrerPolicy="no-referrer"`, an accessible `title`, `loading="lazy"`, and a fixed-height internally-scrolling frame; React's `srcDoc` prop performs attribute escaping (no manual string interpolation into innerHTML).
- **ITEM-4**: Streaming/incomplete safety — when the custom renderer's `isIncomplete` prop is true (fence still streaming), force the Code view and disable the Preview toggle option, so a half-written `<script>`/tag can never be rendered mid-stream.
- **ITEM-5**: Header affordances on the `HtmlBlock` — a `html` language label (**Lang** affordance) and a copy-source button `data-testid="html-block-copy-btn"` (**Copy** affordance) using the existing `Button` + `message` kit, mirroring the syntax extension's copy pattern.
- **ITEM-6**: Wire the renderer into Streamdown once via a shared `streamdownPlugins` (`PluginConfig`) const in `chat/core/utils/streamdownPlugins.ts`, and pass `plugins={streamdownPlugins}` on BOTH `<Streamdown>` instances (`chat/components/TextContent.tsx` and `chat/extensions/text/components/TextContent.tsx`) — the single chokepoint both renderers share.
- **ITEM-7**: Gallery combos — add a ```` ```html ```` block to the `renderingShowcase` fixture (`chat-deep.ts`) so the `deep-chat-rendering-showcase` surface renders the Code (default) mode, and add an `html-preview` interaction recipe to that deep-state (`deepStates.tsx`) that clicks the toggle to capture the Preview (render) mode — both modes covered.
- **ITEM-8**: Affordance-audit detector + docs — add a guarding (non-allowlisted) `html-render` rule to `scripts/affordance-audit.mjs` asserting the `html-block-toggle` control under each `html-block` container, and update `docs/AFFORDANCE_MATRIX.md` (move G3 from §5b gaps to §5a covered, add the §6 detector-table row, mark §7(b) shipped).

## Files to touch

- `src-app/ui/src/modules/chat/core/utils/HtmlBlock.tsx` — **new** (ITEM-1, ITEM-3, ITEM-4, ITEM-5)
- `src-app/ui/src/modules/chat/core/utils/htmlBlockSandbox.ts` — **new** (ITEM-2)
- `src-app/ui/src/modules/chat/core/utils/streamdownPlugins.ts` — **new** (ITEM-6)
- `src-app/ui/src/modules/chat/components/TextContent.tsx` — edit: pass `plugins` (ITEM-6)
- `src-app/ui/src/modules/chat/extensions/text/components/TextContent.tsx` — edit: pass `plugins` (ITEM-6)
- `src-app/ui/src/dev/gallery/fixtures/chat-deep.ts` — edit: add html block to renderingShowcase (ITEM-7)
- `src-app/ui/src/dev/gallery/deepStates.tsx` — edit: add `html-preview` interaction (ITEM-7)
- `src-app/ui/scripts/affordance-audit.mjs` — edit: add `html-render` rule (ITEM-8)
- `src-app/ui/docs/AFFORDANCE_MATRIX.md` — edit: G3 covered / §6 / §7 (ITEM-8)
- `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts` — **new** (tests, Phase 3/8)
- `src-app/ui/src/dev/gallery/coverage.ts` — edit: `HtmlBlock` coverage entry (added per DRIFT-1.1; tsc-enforced coverage map)
- `src-app/ui/src/components/ui/testIds.generated.ts` — **regenerated** (`gen:testid-registry`; new testids — DEC-8 / DRIFT-1.2)
- `src-app/ui/src/dev/gallery/stateMatrix.generated.ts` + `STATE_MATRIX.md` — **regenerated** (`gen:state-matrix`; DRIFT-1.2)
- `src-app/ui/src/dev/gallery/galleryCoverage.generated.ts` — **regenerated** (`gen:gallery-coverage`; DRIFT-1.2)

## Patterns to follow

- **Custom renderer seam** — Streamdown 2.5.0 `plugins.renderers` (`{ component, language }`); component props `{ code, isIncomplete, language, meta }`. Verified against `node_modules/streamdown/dist/chunk-*.js` (`t.renderers.find(...language...)` → `jsx(component,{code,isIncomplete,language,meta})`). The mermaid plugin is the in-package reference for this hook.
- **Copy affordance + toast** — mirror `chat/extensions/syntax/extension.tsx` `CodeBlock` (`navigator.clipboard.writeText` + `message.success/error`, `Check`/`Copy` lucide icons, `data-testid` on the button).
- **Toggle control** — the kit `Segmented` (`components/ui/kit/segmented.tsx`): `data-testid` on root + `${testid}-opt-<value>` per trigger + `data-state="on|off"`; `defaultValue` for uncontrolled default-Code.
- **Streamdown wiring** — mirror the existing `shikiTheme` / `components` / `urlTransform` prop passing in both `TextContent.tsx` files; add `plugins` beside them. Existing image-exfil guard (`streamdownUrlTransform.ts`, `useStreamdownComponents` `img`) is the precedent for "safety guard co-located with the Streamdown renderers".
- **Gallery deep-state + interaction** — mirror `deepStates.tsx` `deep-chat-long` `interactions[]` recipe shape (`{ name, note, steps: async d => { await d.click(testid) } }`) and the `renderingShowcase` fixture block-list in `chat-deep.ts`.
- **Affordance detector rule** — mirror the existing `RULES[]` entries in `scripts/affordance-audit.mjs` (`{ name, label, container, control }`); NO allowlist entry (the feature ships, so the rule guards).
- **E2E** — mirror `tests/e2e/chat/markdown-rendering.spec.ts` (mock SSE via `mockChatStream`/`mockGetMessages`, `seedAssistantWithText`, assistant-bubble assertions) — no real LLM.
- **Design tokens** — semantic tokens only per `DESIGN_SYSTEM.md` (`bg-muted`, `text-muted-foreground`, radius/spacing scale); no raw hex/`bg-blue-*`.
