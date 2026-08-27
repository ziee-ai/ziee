# DRIFT-1 — upstream-port

Implementation vs plan, written as each item landed. For a port UPWARD, "drift" means:
did the change that actually landed in upstream differ from what the plan said, and
from what the paws original did — and if so, was that deliberate?

- **DRIFT-1.1** — verdict: none — ITEM-1. Both hunks landed exactly as planned
  (`enable_popout_windows: false`, `#[cfg(debug_assertions)]` on the axum import).
  Verified RED first: `cargo check -p ziee-desktop` on untouched `upstream/main`
  fails `error[E0063]`. Verified GREEN after.

- **DRIFT-1.2** — verdict: impl-wins — ITEM-2's COMMENT had to be rewritten, not
  copied. paws' `desktop_cors_config` doc says "the header list is now belt-and-braces
  rather than load-bearing: `ziee::create_cors_layer` unions
  `REQUIRED_CUSTOM_REQUEST_HEADERS` in regardless". Upstream has no union (DEC-5), so
  here the list IS load-bearing and that comment would be actively false. Rewritten to
  describe upstream's actual situation. The plan said "hand-written against upstream's
  shape" but did not anticipate that a copied COMMENT can be the thing that is wrong;
  recorded because it is the exact failure mode this branch's own ITEM-11 is fixing
  elsewhere.

- **DRIFT-1.3** — verdict: impl-wins — ITEM-2 gained a hunk the plan did not list.
  To let the desktop allowlist reference the handler's constant rather than re-spell
  the literal, `CHAT_STREAM_CONNECTION_HEADER` had to become `pub` AND be re-exported
  from `lib.rs` (the desktop is a separate crate). The plan named only the `pub`.
  Additive, no caller changes; `PLAN.md` *Files to touch* amended.

- **DRIFT-1.4** — verdict: plan-wins — ITEM-3's rationale is NOT paws'. paws justifies
  keeping the settings REST mounted because its admin UI module is not hidden — a
  reason that does not exist upstream, which hides nothing. Rather than copy a
  justification that would be meaningless here, the code states the reason that IS
  true upstream: those routes only read and write configuration, nothing there
  egresses a query, and the admin page should keep working on a disabled deployment.
  Same split, honest reason.

- **DRIFT-1.5** — verdict: plan-wins — ITEM-3 takes only PART of paws' voice change.
  paws' `voice/mod.rs` diff contains two things: the fail-closed `new()` default (in
  scope, INV-3) and the `voice_enabled()` default FLIP (paws product, excluded). Only
  the first is here. Verified by reading upstream's `voice/mod.rs` and diffing paws'
  against it before touching anything.

- **DRIFT-1.6** — verdict: resolved — ITEM-5 could NOT be taken as a file. paws'
  `uploads.rs` contains the LFS forwarder (in scope) AND a
  `repository.git_credential()` extraction that depends on a `models.rs` method paws
  added for its default-model work (out of scope). Taking the file wholesale would not
  have compiled and would have dragged paws product code upstream. Hand-ported the two
  LFS hunks only. This is the concrete case DEC-3 exists for.

- **DRIFT-1.7** — verdict: resolved — ITEM-4/ITEM-6 required scrubbing paws lifecycle
  IDs. The ported files referenced `TEST-8`, `TEST-9`, `TEST-10`, `INV-3`, `audit
  FIX-5`, `audit FIX-6`, `audit round 2/3` and `DEC-12` — identifiers that resolve to
  nothing upstream. Each was rewritten to name the thing instead of the ID (e.g.
  "pinned by that module's `wire_shape_tests`"), and a `const _TEST_ID` that existed
  only to satisfy paws' A11 gate was deleted outright.

- **DRIFT-1.8** — verdict: impl-wins — **ITEM-11 did not exist at plan time.** While
  verifying the port I ran two unit suites and found two tests RED on `upstream/main`
  itself. Fixing them is not a port and was not planned. Added as ITEM-11 with its own
  commit so it can be dropped independently. The plan was incomplete, not wrong; PLAN
  and BASE amended to record both the item and the measurement.

- **DRIFT-1.9** — verdict: none — ITEM-10's assertions hold.
  `git diff upstream/main...HEAD --stat -- sdk agent-kit src-app/server/vendor/pgvector
  .github src-app/ui/openapi src-app/desktop/ui` is EMPTY: no submodule pointer moved,
  no generated OpenAPI/types drift, no paws CI, and the desktop UI workspace untouched.
  `src-app/Cargo.lock`'s only delta is the single `tower` line the dev-dep requires.

- **DRIFT-1.10** — verdict: none — the whole workspace compiles
  (`cargo check --workspace --all-targets`, exit 0) and the targeted unit tests pass,
  including the CORS preflight test verified RED by mutation.

**Unresolved drifts:** 0
