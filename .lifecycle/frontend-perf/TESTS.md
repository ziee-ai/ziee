# frontend-perf — TESTS (living, per-item)

Two test kinds per item: **(A) a measured perf assertion** (the win is real) and
**(B) a functional-regression e2e** (the app still works after the load-timing
change). Perf assertions are captured via the `.runlogs/perf-record.mjs` harness
and, for the durable ones, codified as ITEM-8's committed budget check. e2e specs
run on the real prod build (no `page.route` API mocking — rule 14).

- **TEST-1** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/perf/streamdown-lazy-render.spec.ts` — asserts: on the seeded "Rendering Showcase" conversation, a fenced code block renders WITH Shiki syntax highlighting (tokenized spans), a `$…$`/`$$…$$` math expression renders as KaTeX (`.katex` DOM), and a ```mermaid block renders the MermaidBlock toggle — i.e. the now-lazy plugin chunk loads + mounts and rendering is unchanged.
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/ui/scripts/check-bundle-budget.mjs` — asserts: the built entry chunk no longer contains the katex/shiki plugin graph (entry-chunk gzip drops below the post-ITEM-1 threshold), i.e. the plugin objects are not statically reachable from the entry; this build-artifact check backstops the e2e render proof and guards the leak from returning.
- **TEST-3** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/perf/login-prefetch-scope.spec.ts` — asserts: the unauthenticated login page downloads ≤ 3 JS chunks (not ~80), AND after login the chat route still loads its chunk and renders, AND navigating to a settings page still loads that page's chunk on demand.
- **TEST-4** (tier: e2e) [covers: ITEM-3, ITEM-4] file: `src-app/ui/tests/e2e/perf/route-chunks-load.spec.ts` — asserts: every split-out module/route still reaches its chunk and renders when navigated to (admin + non-admin), proving the eager→lazy split and the ineffective-dynamic-import fixes did not break any route's mount.
- **TEST-5** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/perf/drawer-nofetch.spec.ts` — asserts: on the chat route as admin, with the LLM-repository drawer CLOSED, no `GET /api/llm-repositories` request fires; opening the drawer then fires it and renders the list.
- **TEST-6** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/perf/boot-waterfall.spec.ts` — asserts: on an authenticated cold load, the chat route chunk request and/or the sync SSE connection begins before `/auth/me` resolves (recorded request ordering), and the chat route still renders correctly.
- **TEST-7** (tier: unit) [covers: ITEM-7] file: `src-app/ui/scripts/check-single-date-lib.test` (or the ITEM-8 budget script) — asserts: only ONE date library resolves into the entry chunk after consolidation; no functional date-format regression (covered incidentally by existing timestamp e2e).
- **TEST-8** (tier: unit) [covers: ITEM-8] file: `src-app/ui/scripts/check-bundle-budget.mjs` (self-test) — asserts: the budget script fails when the built entry-chunk gzip exceeds the threshold or the login chunk count exceeds its cap, and passes on the optimized build; wired into a `check:*` npm script so a regression is caught in `npm run check`.

Note: no new permission is introduced (pure load-timing refactor), so no
`[negative-perm]` restricted-user e2e is required (A10 N/A). No backend diff, so
no backend deny test (A9 N/A). UI diff ⇒ ≥1 `tier: e2e` present (TEST-1,3,4,5,6).
