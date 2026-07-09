# TEST_RESULTS — js-tool-scripting

Phase 8 in progress. The **unit + integration tiers are complete and green**;
the **e2e tier is pending** (the next step, per the requested integration-first
split). Backend diff → the cargo tiers apply; ui diff → the frontend gate applies.

## Frontend static gate (ui workspace)

- npm run check (ui): PASS

## Unit tier (lib `#[cfg(test)]`, all green)

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-10**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS
- **TEST-16**: PASS
- **TEST-17**: PASS
- **TEST-20**: PASS
- **TEST-21**: PASS
- **TEST-23**: PASS
- **TEST-26**: PASS
- **TEST-29**: PASS
- **TEST-37**: PASS

## Integration tier (`tests/js_tool/mod.rs`, 6 tests green — `--test-threads=1`)

- **TEST-9**: PASS
- **TEST-15**: PASS
- **TEST-18**: PASS
- **TEST-19**: PASS
- **TEST-25**: PASS
- **TEST-27**: PASS
- **TEST-28**: PASS

## E2E tier — PENDING (next step)

Not yet authored/run (needs the Playwright + gallery + docker stack):

- **TEST-11**: PENDING (approve round-trip)
- **TEST-12**: PENDING (deny round-trip)
- **TEST-30**: PENDING (resolve POST)
- **TEST-31**: PENDING (inner-approval flow)
- **TEST-32**: PENDING (history shows `script` source)
- **TEST-33**: PENDING (gallery deep-states render clean)
- **TEST-35**: PENDING (primary run_js flow, mocked SSE)
- **TEST-36**: PENDING (real-LLM, provider-agnostic; soft-skip)

The phase-8 gate stays red until every e2e-tier spec is authored + PASS.
