# DECISIONS — Mermaid code⇄render toggle

Every human/product input resolved up front so implementation runs nonstop. All
resolved by convention against the existing codebase; none needed escalation
(the parent task pre-resolved the two policy questions — segmented/ghost control
+ J7 placement).

### DEC-1: What control renders the source⇄render toggle?
**Resolution:** the kit `Segmented` control (`src/components/ui/kit/segmented.tsx`) with two options `Diagram` (value `render`) and `Source` (value `source`), `size="sm"`, mandatory `data-testid="mermaid-source-toggle"`.
**Basis:** user — the parent task fixes "toggle = the app's standard segmented/ghost control per Spec B variant policy"; `Segmented` is that standard control (a single-select Tabs-without-panels).

### DEC-2: Where does the toggle + button cluster sit in the block?
**Resolution:** in the block's always-visible header bar (same corner as the other block actions) — the Segmented leads the left, the copy-source + download-svg ghost icon buttons sit right-aligned, mirroring `MarkdownTable`'s header toolbar.
**Basis:** user — the parent task fixes placement "per J7 = same corner as other block actions"; `MarkdownTable.tsx` is the concrete in-repo realization of that corner.

### DEC-3: Which mode is the default?
**Resolution:** `render` (show the diagram) by default; toggling reveals source. On an invalid diagram we STAY in render mode but show an inline error (source remains one click away) rather than auto-switching to source.
**Basis:** convention — AFFORDANCE_MATRIX §4 row 2 user story ("Show me the diagram, not the source") + Rndr is the REQUIRED default; auto-switching modes would fight the user's toggle and hide the intended primary view.

### DEC-4: How is the mermaid diagram rendered, and how is theme handled?
**Resolution:** the `mermaid` npm package (already a declared dep `^11.15.0` in both ui workspaces), dynamic-imported inside the render effect; `mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme: isDark ? 'dark' : 'default' })` then `await mermaid.render(id, code)`; `isDark` from `useThemeOptional()`. Re-render on `code` or `isDark` change.
**Basis:** convention — mirrors Streamdown's own lazy-mermaid + strict-security approach; `useThemeOptional` is the app's theme hook and tolerates a provider-less gallery case.

### DEC-5: How is the source view rendered (and why not Streamdown's CodeBlock)?
**Resolution:** a plain `<pre><code>` mono block inside the `bg-background` body — NOT Streamdown's `CodeBlock`.
**Basis:** codebase — Streamdown's `CodeBlock` stamps `[data-streamdown="code-block"]`, which the affordance detector's `code-copy` rule requires to contain a copy button; emitting a bare one (copy lives in our toolbar) would trip that guard. A `<pre>` avoids the marker; copy-source is provided in the toolbar for both modes.

### DEC-6: When is the diagram rendered relative to the active mode, and how are streaming / errors handled?
**Resolution:** render whenever `!isIncomplete && code.trim()`, regardless of active mode (so switching is instant and download-svg works in both modes); while `isIncomplete` (streaming) show a deferred placeholder and do NOT call `mermaid.render`; on a parse throw, catch internally and render an inline error state (never rethrow — the block is inside `StreamdownErrorBoundary`); a cancellation guard discards stale async SVG when `code`/theme change mid-render.
**Basis:** codebase — Streamdown defers incomplete fences (`useIsCodeFenceIncomplete`) and wraps each renderer in an error boundary; internal catch keeps one bad diagram from blanking the whole message.

### DEC-7: What test tier covers this UI feature?
**Resolution:** Playwright e2e against the component gallery for all behavioral assertions, plus the affordance-audit detector run; NO new unit-test runner.
**Basis:** codebase — the `src-app/ui` workspace ships no unit runner (vitest lives only in `desktop/ui`); its established tier is e2e-against-the-gallery + `npm run check` static gates. Adding a runner just for this feature would be scope creep with nothing to execute it in phase 8.

### DEC-8: Does the diagram get a `<svg>` DOM injection method, and is it safe?
**Resolution:** inject the mermaid-produced SVG string via `dangerouslySetInnerHTML` into the render-mode body.
**Basis:** convention — identical to Streamdown's built-in Mermaid component; `securityLevel: 'strict'` makes mermaid sanitize the SVG (strips scripts/HTML labels), so the injected markup is trusted output of the sanitizer, not user HTML.

### DEC-9: Download-SVG filename?
**Resolution:** `mermaid-diagram.svg` (a fixed, human-obvious name), `image/svg+xml` blob via the object-URL `<a download>` idiom.
**Basis:** convention — matches the export-extension's fixed-name download pattern (`conversation-<id>.json`); a diagram has no natural title to derive from.

### DEC-10: Do the desktop workspace files need duplicating?
**Resolution:** No. Wire only the `src-app/ui` files; desktop inherits via its `@/*` → `../../ui/src` alias. Verify desktop `tsc`/`npm run check` still passes as diligence.
**Basis:** codebase — `desktop/ui/vite.config.ts` + tsconfig resolve `@/*` to the shared ui source; the chat module, kit, and MermaidBlock are shared, not mirrored.
