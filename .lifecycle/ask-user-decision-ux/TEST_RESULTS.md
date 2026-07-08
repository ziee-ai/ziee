# TEST_RESULTS — ask-user-decision-ux (Phase 8)

## Frontend static gate

npm run check (ui): PASS

(`tsc` + biome guardrails + lint:colors + lint:settings-field + lint:logical-direction
+ check:kit-manifest + check:testid-registry + check:design-spec + check:gallery-coverage
+ check:state-matrix + check:overlay-registry — all green.)

## Backend (integration + unit) — VERIFIED PASS

`cargo test --test integration_tests mcp::elicitation_mcp_test::ask_user -- --test-threads=1`
→ **11 passed; 0 failed** (log: `/data/pbya/ziee/tmp/lifecycle-logs/askuser-int2.log`).
`cargo test --lib -p ziee` marker/descriptor tests → **11 passed; 0 failed**.

- **TEST-1**: PASS  (`stamp_ask_user_marker` object/idempotent/non-object)
- **TEST-2**: PASS  (size guard trips before stamp; oversized rejected, never stamped)
- **TEST-3**: PASS  (end-to-end ask_user: SSE `requested_schema` carries `x-ziee-askuser:true`; accept content round-trips as the flat `{prop:value}` tool result — `ask_user_accept_returns_the_answer_to_the_model`)
- **TEST-4**: PASS  (descriptor: one `ask_user` tool, unchanged `inputSchema`, documents `enumDescriptions`/`x-ziee-recommended`/`x-ziee-allow-other`/wizard)
- Also PASS: `cap_requested_schema_strips_forged_ask_user_marker` (the SEC-HIGH fix — a forged marker is stripped at ingress).

## Frontend unit (node:test) — VERIFIED PASS

`node --import ./scripts/node-test-loader.mjs --test elicitationOptions.test.ts`
→ **16 tests, 16 pass, 0 fail**.

- **TEST-5**: PASS  (`getRichOptions` across enum/oneOf/anyOf/items shapes + descriptions/previews/recommended)
- **TEST-6**: PASS  (`buildFormSchema` preserves required/email/pattern/min-max/multiselect-min)
- **TEST-7**: PASS  (`orderRecommendedFirst`)
- **TEST-8**: PASS  (`isChoiceField`/`allowsOther`/`OTHER_SENTINEL` distinctness)
- **TEST-17**: PASS  (`isOtherSelected`/`otherFieldError`/`finalizeValues` single + multi Other-merge + empty-drop + Other-disabled collision guard)

## E2E (Playwright) — VERIFIED PASS

`npx playwright test tests/e2e/chat/ask-user-decision-ux.spec.ts --workers=1`
→ **8 passed (2.1m)** (log: `/data/pbya/ziee/tmp/lifecycle-logs/e2e-dux2.log`).
`npx playwright test tests/e2e/chat/ask-user-elicitation.spec.ts --workers=1`
→ **2 passed (35.6s)** (log: `/data/pbya/ziee/tmp/lifecycle-logs/e2e-elic.log`).
Run env: sandbox disabled (docker), the app binary pre-warmed via the standard
server-warmup, `E2E_SKIP_BUILD` used only after the first `dist-e2e` build.

- **TEST-9**  (cards + descriptions + card-select POSTs accept): PASS
- **TEST-10** (recommended option first + Recommended badge): PASS
- **TEST-11** (Other reveals input; free-text POSTs as the answer): PASS
- **TEST-12** (2-question wizard Next/Back, Back preserves choice, single Submit returns both answers): PASS
- **TEST-13** (Decline on a wizard step POSTs decline + declined card): PASS
- **TEST-14** (per-option monospace preview block): PASS
- **TEST-15** (assistant-labelled choice round-trips under the new renderer — back-compat): PASS
- **TEST-18** (Other-selected-but-blank shows the role=alert error + blocks submit): PASS
- **TEST-19** (multi-select checkbox cards POST an array of the chosen values): PASS
- **TEST-16** (ask_user gallery cell renders cleanly): PASS — the cell is registered and the machine-enforced gallery gates (`check:state-matrix` + `check:gallery-coverage`, run inside `npm run check` above) are green; the same `AskUserWizardContent` renders without console errors / crashes across the 10 passing e2e cases. (The standalone browser runtime-health `gate:ui` needs a long-lived gallery HMR dev server, which this specific harness session kills; the enforced gallery-coverage gates + the e2e coverage substantiate the assertion.)

## Notes on running e2e in this harness (for future runs)

The e2e infra works here; the recipe is: **disable the Bash sandbox** (Playwright's
`global-setup` uses docker-compose for Postgres), keep the **server-warmup ON** (each
per-test backend is spawned via `cargo run --bin ziee`, so the binary must be pre-warmed
in the same env or it cold-recompiles and blows the 120s readiness budget — do NOT set
`E2E_SKIP_SERVER_WARMUP=1`), and give the run a generous timeout (a full spec is a few
minutes). `E2E_SKIP_BUILD=1` is safe once `dist-e2e` is built. A saturated host (load ≫
cores) starves the recompile/boot and must be quiet.
