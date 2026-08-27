# FIX_ROUND-1 — upstream-port

Two blind angles that differ in kind — **design-conformance** (required by the skill)
and **correctness/concurrency-resource** — each given only `git diff
upstream/main...HEAD` and, for the conformance angle, a statement of the intent and
constraints. 12 confirmed findings, 1 rejected.

## The three that mattered

**1. My own guard was over-sized, and my own comment was false.** The js_tool
kill-switch guard returned the bare router, and I had written that "js_tool's entire
route surface IS the JSON-RPC endpoint" to justify it. `routes.rs`'s module doc says
otherwise **on line 2**: it also carries the admin `api_route` `/js-tool/settings`. So
`js_tool: { enabled: false }` 404'd the admin settings page — a capability removal, in
a PR whose binding constraint is not to remove capabilities, and asymmetric with the
split I had deliberately applied to web_search and lit_search *in the same commit*.
Split js_tool the same way.

**2. The voice hunk was out of scope, so it is gone.** Upstream's `VoiceModule` already
had both the `unwrap_or(true)` default and the `register_routes` guard
(`voice/mod.rs:126-132`). Flipping its `new()` initialiser to `false` therefore fixed
no defect this branch names, and was the precise shape this port must not carry: a
default changed rather than a guard added. `voice/mod.rs` is reverted to
`upstream/main` and the module is untouched. `DESIGN_FIDELITY`'s INV-3 verdict is
amended to say the fail-closed default applies to the three modules that GAIN the
field, and that voice is deliberately excluded — rather than leaving the earlier
claim, which read as if paws' voice change had been partially adopted.

**3. The security fix had no test — the biggest real gap, and the one this process
exists to catch.** `TestServerOptions` already had a `voice_enabled` knob and voice had
a dedicated `config_gate_test`; the kill-switch fix, which is the security-shaped part
of this branch, had neither. It does now, and the test is built so that neither half of
the contract can pass by accident:

- the JSON-RPC route must be **404**, not merely non-200 — a "not 200" assertion would
  pass against the ORIGINAL bug, where the route was mounted and answered 403 to a
  permissionless caller;
- the settings route must **not** be 404 — which is exactly what finding 1 got wrong,
  so this assertion would have caught it;
- and a **positive control** that with defaults (no config section) every endpoint is
  still mounted, without which all of the above passes vacuously against a build in
  which the routes never existed. That control is also what pins this port's promise to
  existing deployments: adding the guard changes nothing for anyone who has not set the
  switch.

All 4 pass.

## The one I rejected, by running it

The conformance angle argued the `default_example()` change in `background_mcp` was
over-reach because "no test requires it" — the battery maps the absent-`spec` case to
`Ok(None)` before any message assertion. Rather than argue, I restored `upstream/main`'s
`tools.rs` and ran the test: `every_spawn_refusal_is_actionable` **panics at
tools.rs:2018**. The change is load-bearing; the finding's premise was wrong (the
battery is not the only assertion — `tools.rs:1904-1915` separately requires the message
to contain `default_example()`). Kept, and recorded as `triage: rejected` with the
evidence rather than quietly ignored.

## Two more, both verified before acting

- `CHAT_STREAM_CONNECTION_HEADER` was exported with the stated rationale "so allowlists
  can reference the constant instead of re-spelling the literal" — and the production
  allowlist still re-spelled it. Only the test used the constant. Fixed.
- The comment claiming this was "the ONLY SSE endpoint without keep-alive" is false;
  `code_sandbox/streaming.rs:250` also lacks one. Corrected in both places. That stream
  is bounded by a single command rather than idling indefinitely, so it is left alone —
  but the sweep is no longer claimed to be complete.

## The five carried, with reasons

Four `lfs_progress.rs` findings (the forwarder writes the 0/0 LFS scan frame and can
clobber the bar; a fast cached step throttles away everything after it; `phase` is
hardcoded; the channel is still unbounded during a DB stall) plus the
`MACOSX_DEPLOYMENT_TARGET` floor-raise. All are **confirmed** and all are **wontfix
here**, for one shared reason: they describe code that already ships in paws, so this
is a faithful port rather than a defect introduced here, and fixing them blind — with
no rootfs, no squashfuse, and no multi-GB LFS repo on this box — would be guessing at
behaviour I cannot reproduce. Reported to the owner for a paws-side branch where they
can be. The macOS floor-raise has no alternative that builds at all, so it is surfaced
in the PR body as a packaging decision rather than silently changed.

## Termination

Round 1: conformance 9, correctness 5, overlap 0 — **below the estimator's ≥2
corroborated floor**, so Chapman T1 declines and the decay rule decides alone. Round 2
therefore runs, scoped to this round's diff.

**New confirmed findings:** 12 (1 further rejected)
