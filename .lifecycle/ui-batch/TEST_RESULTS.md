# TEST_RESULTS — ui-batch

Every TEST-ID from TESTS.md, with what was actually run. Nothing here is
reported as passing unless it was executed and observed.

## Results

- **TEST-1**: PASS — `node --import ./scripts/node-test-loader.mjs --test src/modules/chat/core/stores/SplitView.store.test.ts` → `# pass 17 / # fail 0`, including `ok 16 - reset() makes a subsequent auto-open a plain navigate, not a pane replace`. Re-run after the fix round.
- **TEST-2**: PASS — gallery visual, both themes (light + dark).
- **TEST-3**: PASS
- **TEST-4**: PASS — and independently proven non-vacuous: with the rejected `max-w-[60%]` ceiling temporarily restored, its new gutter assertion FAILS with *"64.4px of empty space sits between the "+" button and an ELLIPSIZED model name"*. Reverted immediately after.
- **TEST-5**: PASS
- **TEST-6**: PASS — real app shell; the three sidebar captions share one left edge.
- **TEST-7**: PASS — every menu row shares its own edge, strictly outdented from the captions.
- **TEST-8**: PASS (flaky) — Playwright reports `1 flaky`: two attempts died in `loginAsAdmin` waiting for the login form, the third passed in 1.2m. See "login flakiness" below.
- **TEST-9**: PARTIAL — reaches and passes every assertion this branch could affect, then fails on the one that needs a real model reply. Proven, not asserted: on retry #2 login succeeded and the spec ran to `:63` `expect(pane1.locator('[data-role="user"]')).toBeVisible()` **PASSED** — the split was built, the in-pane composer opened, the message sent, and the conversation ADOPTED INTO PANE 1 with the split intact — then failed at `:64` `[data-role="assistant"]` after 60s. That is the regression control satisfied for everything in scope. See "TEST-9" below.
- **TEST-10**: PASS — first attempt, no retry. The project surface collapses the split exactly like the sidebar path.

Run: `sg docker -c "npx playwright test --config=<ui>/playwright.config.ts <specs> --workers=1"`.
The harness brought up its own Postgres (`ziee-tailtest-postgres-7b17b8d1`, port
54332) and left the concurrent `ziee-review-general` stack untouched.

`npm run test:unit` (whole suite): 468 pass / 8 fail. All 8 failures are
pre-existing and unrelated to this diff — see "Pre-existing gate failures".

Visual suite command (TEST-2…TEST-5):
`GALLERY_PORT=1437 npx playwright test -c playwright.visual.config.ts composer-model-selector --workers=1`
→ `5 passed`. Re-run after every code change in the fix round.

## Frontend gate lines

- `npm run check (ui): FAIL (PRE-EXISTING)` — fails at `check:testid-registry`,
  the FIRST of five artifact gates that are already red on the base commit. See
  below; this diff does not cause or worsen any of them.
- `gate:ui (ui): PARTIAL` — `tsc` **PASS**, `lint` **PASS**, `visual`
  skipped (`--skip-visual`), `runtime-health` **UNRELIABLE in this
  environment** (evidence below).

## Correction — the e2e suite was never actually blocked

An earlier revision of this file recorded TEST-6…TEST-10 as blocked because the
shell could not reach the docker socket, and proposed a group change to unblock
them. **That was wrong, and it was my inference rather than a verified fact.**
The correct, established pattern is to activate the group for a single
invocation:

```
sg docker -c "npx playwright test --config=… <specs> --workers=1"
```

No group membership change is needed or wanted. The specs were unattempted, not
unrunnable, and the four written for this branch now have real verdicts above.

One further self-inflicted failure is worth recording: the first `sg docker` run
still failed, with every spec dying in ~80ms on `TEST_RUN_ID not set`. That was
not docker either — the shell's cwd is the repo ROOT, so `npx playwright test`
resolved a different config with no `globalSetup`. Passing
`--config=<ui>/playwright.config.ts` fixed it.

## Login flakiness (affects TEST-8, and TEST-9 below)

`loginAsAdmin` (`tests/common/auth-helpers.ts`) intermittently times out after
30s waiting for either `app-setup-username-input` (:103) or
`auth-login-username` (:144) — i.e. the app renders neither the first-run setup
form nor the login form within the budget. It fails *before* any assertion in
the spec under test.

Evidence that it is the helper and not this branch:

- In the 5-spec run the FIRST TWO specs failed this way and the LAST THREE
  passed, all using the same helper against the same infrastructure.
- Run alone on a fresh database, TEST-8 waited for the LOGIN form rather than
  the SETUP form — the helper believed an admin already existed on a wiped DB.
- TEST-8 then PASSED on retry #2 with no code change (`1 flaky`, exit 0).
- The host was heavily loaded throughout (7-18% idle CPU, load average 290-400,
  ~25 concurrent agent processes from other sessions).

## TEST-9 — the pre-existing control, and why it cannot pass here

TEST-9 is `new-chat-adopt.spec.ts`, which this branch does not modify. It is the
paired control proving the in-split "new chat pane" flow still adopts in place.
It cannot pass in this environment for a reason independent of the diff:

`new-chat-adopt.spec.ts:64` asserts `[data-role="assistant"]` becomes visible
within 60s — a REAL model reply. `provider-helpers.ts:29-55` points a test
provider at `https://api.openai.com/v1` with the literal key
`sk-test-placeholder` unless `OPENAI_BASE_URL` / `ZIEE_TEST_LLM_BASE_URL` is
set, and no `.env.test` exists in either checkout. So no assistant message can
ever arrive.

The four specs written for this branch deliberately assert `[data-role="user"]`
— the user's own message, which persists regardless of whether the model
answers — which is why TEST-8 and TEST-10 pass here while TEST-9 cannot.

**But the control still did its job.** Run with `--retries=2`, attempt 3 got
past the flaky login and executed the whole flow. It passed:

- `chat-split-btn` → `chat-pane-0` + `chat-pane-1` visible (the split is built);
- `pane-start-new-chat` → `pane-new-chat-greeting` visible (the in-pane
  new-chat composer, the path that must NOT collapse the split);
- send → **`:63` `pane1.locator('[data-role="user"]')` visible — PASSED**.

That last assertion is the one that matters: the created conversation was
adopted INTO PANE 1, the split survived, and the window did not navigate away.
Every behaviour this branch could plausibly have broken is therefore verified.
The failure is the next line, `:64`, waiting 60s for `[data-role="assistant"]`
— a model reply that cannot arrive without a bridge.

So TEST-9 is recorded as PARTIAL rather than PASS (it did not go green) or
FAIL (nothing it proves about this branch is broken). Configure a bridge to
take it to green.

To run TEST-9 for real, point the bridge at a local OpenAI-compatible endpoint,
e.g. `ZIEE_TEST_LLM_BASE_URL=http://localhost:4000/v1` (a `local-llm-proxy`
container is running on this host), then re-run it with `sg docker -c`.

## runtime-health — why it is reported as unreliable, not as passing or failing

Two runs of `npm run gate:ui --skip-visual` on this branch each reported 10
failing surfaces — but **a completely different set of 10 each time**, with
wildly different counts (run 1: `deep-chat-streaming` + 9 overlays, HIGH 4-6
each; run 2: `settings-users` HIGH 733, `overlay-group-members-drawer` HIGH 474,
…). A failing set that does not reproduce is a flakiness signature, not a
regression.

The cause is in the findings themselves:

```
"detail":"GET http://localhost/modules/settings/module.tsx — net::ERR_NETWORK_CHANGED"
"detail":"Failed to load resource: net::ERR_NETWORK_CHANGED"
```

i.e. the gallery dev server's connections were being dropped mid-sweep. The host
was genuinely saturated at the time — `top` reported **7.5% idle CPU / 79.1%
system**, with 25 concurrent agent processes from other sessions (this is real
contention, not a misleading load average).

The controls that make this conclusive:

1. **A clean baseline passed.** The same gate, same machine, run against an
   untouched checkout of `khoi`: `182/182 PASS`, `✅ GATE PASSED — every UI DONE
   criterion met`. So the gate itself is not broken on the base.
2. **This branch's own surfaces are clean.** In the run that completed
   meaningfully, the only findings on `deep-chat-long-model-name` and
   `deep-chat-overlong-model-name` were 4 × LOW `spacing-grid` (informational,
   never gating, and reporting the same off-grid values the whole app reports).
   Zero HIGH, zero MEDIUM.
3. **None of the failing surfaces is one this diff touches** — in either run.

Honest conclusion: `runtime-health` needs one clean re-run on a quiet host
before the UI DONE criterion can be signed off. `tsc` and `lint` — the other two
gate legs — pass.

## Pre-existing gate failures (NOT caused by this diff)

Both were established by reproducing them **without this branch's code**.

**1. `npm run check` — five stale generated-artifact gates.** Run on a clean,
untouched checkout of `khoi` (`/home/khoi/ziee/ziee`, `git status` clean):

| gate | clean `khoi` | this branch |
|---|---|---|
| `check:testid-registry` | exit 1 | exit 1 |
| `check:gallery-coverage` | exit 1 | exit 1 |
| `check:state-matrix` | exit 1 | exit 1 |
| `check:overlay-registry` | exit 1 | exit 1 |
| `check:override-registry` | exit 1 | exit 1 |

All five stem from the kit's extraction into the `sdk` submodule without
refreshing the generated artifacts. Two further facts:

- The testid registry lives **inside the `sdk` submodule**. Regenerating it
  produces a commit in a different repository plus a pointer bump; pointing at
  an unpushed commit would break every other clone. Additionally proven
  pre-existing by stashing the only two files this feature adds testids to and
  re-running the check on the otherwise-pristine tree — still stale.
- Regenerating the state-matrix / gallery-coverage artifacts here **deletes every
  `components/ui/kit/*` surface** (that directory no longer exists) and then
  breaks `tsc`, because the hand-maintained `coverage.ts` / `stateCoverage.ts`
  still map those keys. That was attempted, measured, and reverted in its own
  commit rather than shipped.

**2. `npm run test:unit` — 8 remaining failures.** This branch's loader fix took
it from **10 failing files to 8** (and executed tests from 456 to 476),
recovering `SplitView.store.test.ts` — which carries TEST-1 — and
`MessageViewState.store.test.ts`. An audit agent independently re-ran the suite
under both hook versions and reproduced 10 → 8, confirming the remaining set is
a strict subset. Those 8 fail on a different, pre-existing cause:
`ERR_UNSUPPORTED_TYPESCRIPT_SYNTAX: TypeScript enum is not supported in
strip-only mode` (from `export enum Permissions` in `api-client/types.ts`) plus
one test-local `TypeError`, across auth / chat-history / scheduler / voice — all
modules this branch does not touch. Fixing them needs enum removal from product
source or a real transpiler in the unit runner.

Because the suite's exit code is dominated by that pre-existing red, TEST-1 is
reported above from its own targeted run rather than from the suite's exit code.
