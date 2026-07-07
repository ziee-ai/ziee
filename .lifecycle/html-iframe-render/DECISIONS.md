# DECISIONS — every design input resolved up front before implementation

Security-sensitive feature: the sandbox posture decisions (DEC-2..DEC-6) are the
core of the threat model and are resolved conservatively.

### DEC-1: Which fence languages trigger the render toggle?
**Resolution:** Exactly `html` (register `plugins.renderers` with `language: 'html'`). Not `svg`, `xml`, `htm`, or `mermaid` (mermaid is a sibling shard / separate backlog item 7a). A future language is a one-line array addition.
**Basis:** convention — the AFFORDANCE_MATRIX gap G3 / backlog §7(b) is scoped to the ```` ```html ```` block; keeping scope to `html` avoids overlap with the parallel mermaid-toggle shard.

### DEC-2: Default view — Code or Preview?
**Resolution:** **Code** (source), via `Segmented` `defaultValue="code"`. The user opts into Preview per block by clicking the toggle.
**Basis:** user — the task mandates "default view = CODE for safety; user opts into render per block". Also defense-in-depth: arbitrary LLM/attacker HTML never auto-executes on message render.

### DEC-3: iframe `sandbox` token set?
**Resolution:** `sandbox="allow-scripts"` — and ONLY that. Explicitly EXCLUDED: `allow-same-origin` (critical), `allow-top-navigation`, `allow-top-navigation-by-user-activation`, `allow-popups`, `allow-popups-to-escape-sandbox`, `allow-forms`, `allow-modals`, `allow-pointer-lock`, `allow-downloads`.
**Basis:** convention (web platform security) — `allow-scripts` WITHOUT `allow-same-origin` runs scripts in an opaque/null origin, so the frame cannot access the parent DOM, cookies, `localStorage`, or `window.parent`/`top` (Same-Origin Policy blocks it). Granting BOTH `allow-scripts` + `allow-same-origin` would let the frame reach in and remove its own `sandbox` attribute — the canonical bypass — so they are never combined. Omitting top-navigation prevents clickjacking/phishing redirects of the host tab; omitting popups/forms/downloads removes exfil + nuisance surfaces.

### DEC-4: How is HTML injected into the iframe — `srcdoc` vs `src=blob:` vs same-origin document?
**Resolution:** React `srcDoc={buildSandboxedSrcdoc(code)}`. No `src`, no `blob:` URL, no `contentWindow.document.write`.
**Basis:** convention — `srcdoc` with a sandboxed frame yields a null origin (no origin inheritance, unlike a same-origin blob/about:blank the parent could script into). Passing via React's `srcDoc` prop means React does the HTML-attribute escaping; we never string-concat user HTML into `innerHTML`/`outerHTML`, eliminating the attribute-escape XSS class on the host side.

### DEC-5: External network policy inside the preview (the "conservative default")?
**Resolution:** **Block all external network.** `buildSandboxedSrcdoc` PREPENDS `<!DOCTYPE html>` + `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; media-src data:; form-action 'none'; base-uri 'none'">` as the LITERAL first bytes of the srcdoc, ahead of ALL user markup (it does NOT splice the meta into the untrusted HTML). A meta before `<html>`/`<head>` is parsed into the implicit head and governs the whole document; because nothing user-provided can precede it, neither a pre-`<head>` `<script>` nor a decoy `<!-- <head> -->` comment can escape or trap the CSP (both were caught + fixed in Phase 7, FIX_ROUND-1). CSP policies are additive so a user-injected looser CSP later cannot relax ours. This allows inline `<script>`/`<style>` and `data:` images/fonts (so the HTML actually renders and is interactive) but blocks ALL external fetch/XHR/WebSocket/EventSource (`connect-src` falls back to `default-src 'none'`), external scripts/styles/images/fonts/media, nested external frames (`frame-src` → none), form submission anywhere, and `<base>` hijack.
**Residual (documented, accepted — surfaced in FIX_ROUND-1):** the CSP blocks external RESOURCE LOADS (fetch/XHR/WS/img/script/style/font/media/form) but cannot block the frame NAVIGATING ITSELF (`location=…` / `<meta refresh>`) — no platform primitive covers same-frame navigation. That self-navigation beacon can carry only attacker-authored data; the null-origin frame has no cookies/storage/parent access, so there is nothing sensitive to leak (isolation, not egress-block, is the guarantee — same posture as server code_sandbox). The module doc + `CSP` comment state this precisely (no "blocks ALL network" overclaim).
**Basis:** user + convention — the task requires "external network per a conservative default — document the decision". Conservative = no external resource load / no phone-home for sensitive data: a prompt-injected/adversarial HTML block cannot load/post to an external origin, and the frame holds nothing sensitive. Rationale mirrors the repo's existing image-exfil guard (`streamdownUrlTransform.ts`) and the code_sandbox "nothing sensitive to exfiltrate" posture. If a future use-case needs external assets, the right answer is an explicit per-render opt-in, never loosening this default. Note: the `sandbox` attr alone does not restrict network; the injected CSP is what severs it — both layers are required.

### DEC-6: Preview frame height / auto-resize?
**Resolution:** Fixed height (`h-96`, 24rem) with the iframe filling it and scrolling internally (`w-full h-96`). NO postMessage-based auto-height.
**Basis:** convention — a cross-origin (null-origin) sandboxed frame cannot be measured from the parent (SOP), and postMessage auto-height would require injecting our own reporter `<script>` into the user's document (an extra attack/interference surface + CSP coupling). A fixed scrolling frame is the minimal, robust choice; resize is a possible future NICE, not required by G3.

### DEC-7: iframe additional hardening attributes?
**Resolution:** `referrerPolicy="no-referrer"` (no URL leak on any allowed sub-request), `loading="lazy"`, `title="HTML preview (sandboxed)"` (a11y accessible name), `className` on a wrapping container carrying `data-testid="html-block"`. No `allow`/`credentialless` needed.
**Basis:** convention — matches least-privilege; `no-referrer` is belt-and-suspenders atop the CSP; `title` satisfies the runtime-health a11y-name check for iframes.

### DEC-8: New testids — registration with `check:testid-registry`?
**Resolution:** Use `html-block`, `html-block-toggle` (+ auto `-opt-code`/`-opt-preview` from `Segmented`), `html-block-copy-btn`. After implementation run `npm run gen:testid-registry` to sync the generated registry; the e2e `tests/e2e/testid.ts` `byTestId` helper needs no change (it takes a raw string).
**Basis:** codebase — `check:testid-registry` (`scripts/gen-testid-registry.mjs --check`) is a generated-snapshot gate; the `gen:` counterpart regenerates it. Same regen discipline as the design-spec gate.

### DEC-9: Gallery generated-snapshot sync (coverage / state-matrix / crawl / fixtures)?
**Resolution:** After the `chat-deep.ts` fixture edit + the `deepStates.tsx` `html-preview` interaction, run `npm run gen:gallery-coverage`, `npm run gen:state-matrix`, `npm run gen:gallery-crawl`, and `npm run gallery:check-fixtures` (fix-forward) so all `--check` gates in `npm run check` pass. Commit the regenerated artifacts.
**Basis:** codebase — these are `gen-*.mjs --check` snapshot gates in `npm run check`; the generated files must be regenerated whenever gallery fixtures/interactions change (same contract as the design-spec regen rule in CLAUDE.md).

### DEC-10: Where does the code-view body get its Shiki highlighting?
**Resolution:** The Code view renders a plain, wrapped `<pre><code>` of the source (no Shiki re-highlighting) inside the `HtmlBlock`, keeping the component self-contained and dependency-light; the source stays readable and copyable. We do NOT re-enter Streamdown's internal `CodeBlock` export (avoids double-header chrome + coupling to a non-public-stable internal).
**Basis:** convention — the syntax-extension `CodeBlock` reference itself renders a plain `<pre><code className={language-...}>` without Shiki; matching it keeps the toggle header single and the component's render deterministic for the e2e source-visibility assertions. (Shiki highlighting of the html source is a possible future NICE; not required by G3, whose REQUIRED cells are Rndr/Tgl/Copy/Lang.)

### DEC-11: Toggle state scope + persistence?
**Resolution:** Local component `useState` keyed by the component instance (one `HtmlBlock` per fence); no store, no persistence across reload. Default recomputed as Code on each mount.
**Basis:** convention — per-block ephemeral UI state (like the footnote `<details>` open state) belongs in local component state, not the Chat store; nothing else needs to observe it.

### DEC-12: Copy toast + clipboard failure handling?
**Resolution:** `navigator.clipboard.writeText(code)` → `message.success('HTML copied to clipboard')`; on reject `message.error('Failed to copy HTML')`. Icon flips `Copy`→`Check` for 2s.
**Basis:** convention — verbatim mirror of the syntax `CodeBlock` copy handler.
