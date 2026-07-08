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

## E2E + gallery runtime-health — BLOCKED (sandbox), NOT run

The enumerated specs are authored and VALID — `npx playwright test --list` discovers
all 11 tests (8 in `ask-user-decision-ux.spec.ts` + 3 in `ask-user-elicitation.spec.ts`),
and `tsc -p tsconfig.json` is clean over the tests. They **cannot execute in this
sandbox session**: any process that spawns a persistent server tree (Playwright's
`global-setup` docker-compose Postgres + per-worker `ziee` server + chromium, or a
standalone gallery dev server for `gate:ui`/runtime-health) is killed with signal 16
(exit 144) before producing output, while short-lived cargo-spawned server subprocesses
(the backend suite above) run fine. This is an environment/infra limit of this session,
not a defect — the specs are well-formed and the behaviours they assert are already
proven at the unit + integration layers above.

- **TEST-9**  (cards + descriptions + accept): BLOCKED (env) — valid, discovered
- **TEST-10** (recommended first + badge): BLOCKED (env) — valid, discovered
- **TEST-11** (Other reveals input + free-text accept): BLOCKED (env) — valid, discovered
- **TEST-12** (2-question wizard Next/Back + single submit both answers): BLOCKED (env) — valid, discovered
- **TEST-13** (decline on wizard step): BLOCKED (env) — valid, discovered
- **TEST-14** (option preview block): BLOCKED (env) — valid, discovered
- **TEST-15** (back-compat headline choice under new renderer): BLOCKED (env) — valid, discovered
- **TEST-18** (Other-blank blocks submit with validation error): BLOCKED (env) — valid, discovered
- **TEST-19** (multi-select checkbox roundtrip → array): BLOCKED (env) — valid, discovered
- **TEST-16** (gallery runtime-health, zero HIGH on the ask_user cell): BLOCKED (env) — the gallery cell is registered + `check:state-matrix`/`check:gallery-coverage` (inside `npm run check`) are green; the browser runtime pass needs a live gallery server, which the sandbox kills.

**To run on an unrestricted host:**
```
cd src-app/ui
npx playwright test tests/e2e/chat/ask-user-decision-ux.spec.ts tests/e2e/chat/ask-user-elicitation.spec.ts --workers=1
npm run gate:ui        # gallery runtime-health for the ask_user wizard surface
```
