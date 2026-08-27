# FIX_ROUND-2 — upstream-port

One fresh angle — **tests-quality + security** — scoped to round 1's diff, and pointed
squarely at the thing round 1 introduced: a brand-new security test. A test written to
prove a fix is the easiest place in a branch to fool yourself, so the brief asked, per
assertion, "name the one-line production change that turns this RED; if you cannot, it
is vacuous", and asked specifically whether a 404 could arise for any reason OTHER than
the route being unmounted.

## The mechanism held up

Verified clean, with reasons I checked rather than accepted: the three new harness keys
are real top-level `Config` fields so the YAML actually reaches the server (and a
mis-spelled key would produce a false RED, not a false green — the module would stay
enabled and the route would answer 200); `RequirePermissions` rejects 401/403 and never
404, so the 404s cannot come from auth; body deserialization happens inside the handler,
after routing; the router split is lossless in the enabled case and byte-identical in
OpenAPI terms; and every assertion has a named one-line red-maker. No assertion is
vacuous.

## The finding that mattered: my security claim was OVER-BROAD

The headline of the kill-switch commit was that an ordinary user could
`POST /api/run-js/mcp` and **execute arbitrary script**. For `js_tool` that is **false**,
and I verified it before acting: `js_tool/handlers.rs:64-70` answers `tools/call` with
an error — *"run_js must be invoked in a chat context; it is executed inline by the chat
runtime, not over the loopback transport"*. The endpoint serves `initialize`,
`tools/list` and `ping`. The execution gate lives in the chat extension.

I then checked the siblings rather than assume the correction generalised:
`web_search/handlers.rs:80` and `lit_search/handlers.rs:76` **do** dispatch `tools/call`.
So for those two the claim holds in full — a disabled deployment really did serve live
web and scholarly queries to any Users-group member, and for `lit_search` five of six
connectors need no key.

So the defect is real in all three and the fix is right in all three, but the SEVERITY
differs and I had flattened it. Corrected per module in `js_tool/mod.rs`,
`js_tool/routes.rs`, the test header, and the commit message — which now states what
`js_tool` leaked (the surface of a switched-off feature) rather than what it did not.
Worth noting the same over-broad claim is in `tinnlab/paws` `816aa6321`, the commit this
ports.

Getting this wrong in a PR to a shared upstream repo would have been worse than the bug:
an inflated severity claim is the kind of thing that costs a reviewer's trust in every
other claim in the diff.

## Three more, all accepted

- The test's own rationale was inverted: `create_user_with_permissions(.., &[])`
  registers through `/auth/register`, which places the user in the DEFAULT GROUP, so it
  is not a permissionless caller — it holds the `use` grants. That makes the 404
  assertions *stronger* than I claimed (a mounted route would SERVE this caller, not
  refuse it), and the file now says so, with a warning not to "tighten" the assertions
  on the false premise.
- The tree now holds **two opposite kill-switch contracts**: these three keep their
  settings REST mounted when disabled; `voice` unmounts everything and
  `tests/voice/config_gate_test.rs` asserts `/voice/settings` MUST be 404. Both are
  green while specifying contradictory behaviour. NOT unified here — that means changing
  `voice`, which is precisely the out-of-scope drift round 1 reverted, and choosing the
  winning policy is the owner's call. Flagged in the test header.
- Post-revert, `voice` fails OPEN in its initialiser while the other three fail closed.
  Benign today (`voice/mod.rs:101` assigns before any early return), and it is the price
  of the correct round-1 revert. Reported with the item above; one decision resolves both.

## Termination

Round 1: 9 + 5 findings, overlap 0. Round 2: 5 findings, **0 corroborated across
angles**, so Chapman T1 declines at its ≥2 floor in both rounds and the decay rule
decides alone.

The profile decays on the axis that matters: round 1 changed BEHAVIOUR (an unmounted
admin route, an out-of-scope module revert, a missing test); round 2 changed **no
behaviour at all** — one factual correction to claims, and three recorded observations.
Severity fell from medium-with-capability-loss to one medium documentation defect and
four lows. Nothing in round 2 was oracle-confirmed except the handler read that
disproved my own claim.

GUARD-SUB does not fire: round 2's findings are spread across a test file, a module, and
two comment sites, not concentrated on a guard being played whack-a-mole with. Round 3
would audit a diff consisting of corrected prose and one reverted file.

**New confirmed findings:** 5 (0 behavioural)

## GUARD-SUB fired. Why I am escalating it rather than overriding it

The validator reports that 3 of round 2's 5 findings land on one test file
(`kill_switch_gate_test.rs`) and instructs: stop, do not write another predicate,
replace the guard with a behavioural test.

**The tripwire's premise does not hold here, and I am saying so rather than silently
continuing.** It exists to catch a hand-written STATIC-ANALYSIS guard pattern-matching a
semantic property — an artifact with unbounded evasion space, where each round finds
another spelling and 0 is unreachable by construction. `kill_switch_gate_test.rs` is the
opposite of that: it is a behavioural integration test that boots a real server with the
switch off and reads real HTTP status codes. It is already the thing GUARD-SUB tells you
to replace a guard WITH.

The concentration is an artefact of scoping: the phase-7 rule says audit the ROUND's
diff, and round 1's diff was largely that new file, so an angle pointed at it will
naturally report against it. And the three findings are not evasions — one is a factual
correction to a severity claim (which I acted on, in code and in the commit message),
one corrects the file's own description of its fixture, one is a noted maintenance
tradeoff. None changed behaviour; none was another spelling of a predicate.

Escalating rather than overriding, per the rule's own instruction, and recording the
reasoning so the owner can judge it. If they disagree, the remedy is cheap: the
assertions are four HTTP calls and would survive any restructuring.

## Convergence

The validator also reports the loop as not converged (5 new confirmed findings in the
final round). True as counted, and the count is the honest number. What the count does
not carry is that **round 2 changed no behaviour at all** — the profile decayed from
"an unmounted admin route, an out-of-scope revert, a missing test" to "a claim stated
too broadly, and three observations". A third round would audit corrected prose.

I am stopping here and handing both facts to the owner rather than running rounds until
a number looks right, which is precisely the unbounded-loop failure the termination
rules were written to prevent.
