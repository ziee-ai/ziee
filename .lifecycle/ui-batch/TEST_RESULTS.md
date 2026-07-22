# TEST_RESULTS — ui-batch

Every TEST-ID from TESTS.md, with what was actually run. Nothing here is
reported as passing unless it was executed and observed.

## Results

- **TEST-1**: PASS — `node --import ./scripts/node-test-loader.mjs --test src/modules/chat/core/stores/SplitView.store.test.ts` → `# pass 17 / # fail 0`, including `ok 16 - reset() makes a subsequent auto-open a plain navigate, not a pane replace`. Re-run after the fix round.
- **TEST-2**: PASS — gallery visual, both themes (light + dark).
- **TEST-3**: PASS
- **TEST-4**: PASS — and independently proven non-vacuous: with the rejected `max-w-[60%]` ceiling temporarily restored, its new gutter assertion FAILS with *"64.4px of empty space sits between the "+" button and an ELLIPSIZED model name"*. Reverted immediately after.
- **TEST-5**: PASS
- **TEST-6**: **NOT RUN** — blocked, see below.
- **TEST-7**: **NOT RUN** — blocked, see below.
- **TEST-8**: **NOT RUN** — blocked, see below.
- **TEST-9**: **NOT RUN** — blocked, see below.
- **TEST-10**: **NOT RUN** — blocked, see below. (Added in round 3, after an audit found the `ProjectDetailPage` fix had no coverage at any tier.)

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

## TEST-6…TEST-9 — blocked, with the exact reason

The full-stack e2e harness provisions a per-run PostgreSQL **container**. This
session's user is not in the `docker` group, so `tests/global-setup.ts` fails at
its very first step:

```
🧹 Cleaning up stale PostgreSQL containers...
permission denied while trying to connect to the docker API at unix:///var/run/docker.sock
```

`id -nG` → `khoi` (no `docker`). There is no supported opt-out: the harness
always creates its own container (that is its per-run isolation design), and
editing `global-setup.ts` to reuse an external database is precisely the shared-
harness workaround B3 forbids.

What WAS verified about these four specs, short of executing them:

- they compile (`tsc` clean) and Playwright resolves and registers all three
  TEST-IDs (`--list` → `Total: 3 tests in 2 files`, plus the existing TEST-9 spec);
- every selector was checked against real source: `#app-sidebar` wraps
  `<LeftSidebar/>` (`sdk/packages/shell/src/layouts/AppLayout.tsx:496`),
  `layout-sidebar-primary-actions-menu-item-new-chat` is used by an existing
  spec, and `conversation-picker-item-*` / `chat-split-btn` / `chat-pane-N`
  come from the sibling split-chat specs;
- the inset arithmetic TEST-6/TEST-7 assert was derived from source and
  independently re-confirmed by two audit agents (captions 12px; every row
  8px + 12px = 20px).

To unblock: `sudo usermod -aG docker $USER`, then a fresh login session.

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
