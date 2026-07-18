# Chunk BG-3 — FIX round 1

Blind multi-angle audit (LEDGER.jsonl — 20 findings across 12 angles incl.
`equivalence`, two `security`, `concurrency`, `build-db`, `wire`, `ordering`,
`feature-unification`, `app-agnostic`) reconciled against every diff hunk
(AUDIT_COVERAGE.tsv — 22 hunks, ≥3 angles each).

Findings surfaced + resolved DURING the drift-convergence loop (all re-verified
before the gate; none deferred):

- **SDK-standalone postgresql_embedded resolution** (build): adding
  `postgresql_embedded` to `ziee-framework` made `cd sdk && cargo check
  --workspace` need `POSTGRESQL_VERSION` (the SDK workspace has no
  `.cargo/config.toml`) — without it the build would download a DIFFERENT default
  PG version and diverge from the shared target. Fixed by adding an
  SDK-standalone-only `sdk/.cargo/config.toml` pinning ziee's exact version
  (TRANSFORMS D6). Re-verified: SDK workspace check exit 0, shared target reused.

- **`env!("ZIEE_POSTGRES_VERSION")` unavailable framework-side** (build): the
  moved `stop_existing_postgres_instance` used the `POSTGRES_VERSION` const, which
  `env!`s a ziee-only build env. Fixed by parameterizing the framework function
  over `pg_ctl_version: &str` (ziee passes the const; the const stays app-side)
  and giving the moved tests a version literal. Re-verified: `cargo test -p
  ziee-framework embedded_pg::` — 2 passed.

- **`set_ignore_missing` on a shared migrator** (types): `Migrator` is not
  `Clone` and `set_ignore_missing` needs `&mut`, so a `&'static Migrator` can't be
  mutated framework-side. Fixed by having ziee own a `LazyLock<Migrator>` that
  applies `set_ignore_missing(true)` once at construction and pass
  `LazyLock::force(&…)` (a `&'static`). Re-verified: `cargo check -p ziee` exit 0.

No other finding required a code change:
- **equivalence** — the lifecycle + boot closure + auth consumers are
  byte-behaviourally identical (same repos/pool/migrator/hooks); E8 golden
  byte-identical + canonical on BOTH surfaces is the machine proof.
- **security** — redacted-URL logging, the CORS/Extension(jwt) re-layer, the
  jti-whitelisted mint, and the owner-`*` posture are all preserved; BootHandle
  is app-internal (no wire exposure).
- **build-db** — grep confirmed zero compile-time `query!` in `embedded_pg.rs`;
  `ziee-framework` stayed build-DB-free.
- **app-agnostic** — the harness names no CODE `ziee::`; skeleton-server
  (framework-only) still builds; ziee provides the `ServerBoot` impl → D-full
  unblocked.

A second blind pass over the full `git diff` hunks (SDK submodule + ziee side,
all 22 in AUDIT_COVERAGE) surfaced no additional equivalence / security / type /
ordering / boundary divergence.

**New confirmed findings: 0**
