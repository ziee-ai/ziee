# Chunk B4 — DRIFT scan (round 1)

Drift = any place the mechanical macro move + path-qualification could diverge
from the pre-extraction behavior/surface. Each candidate reconciled below.

- **DRIFT-1.1** — verdict: none. Generated-symbol surface unchanged. The macro
  still emits `RepositoryFactory`, `init_repositories`, `is_repos_initialized`,
  the `<Type>Repos` Deref wrappers, `ReposAccessor`, and `Repos` INTO ziee's
  `core::repository` (the invocation stayed there). `core::mod.rs`'s
  `pub use repository::{Repos, init_repositories, is_repos_initialized}` resolves
  unchanged. Confirmed: `cargo check -p ziee` (lib+bin) exit 0, zero `Repos.xxx`
  call-site edits.

- **DRIFT-1.2** — verdict: none. Cross-crate path resolution correct. Every
  hard-coded path in the expansion is fully-qualified `::once_cell` / `::sqlx` /
  `::std` / `::paste` / `::tracing`; ziee depends on all five (`Cargo.toml`
  workspace deps), so each is in ziee's extern prelude. `$module_path`/`$type`/
  `$field` are invocation-substituted, so `crate::modules::*` still points at ziee.
  Confirmed: the macro expands with no path/name-resolution error.

- **DRIFT-1.3** — verdict: none. The three deleted `use` lines
  (`once_cell::sync::OnceCell`, `sqlx::PgPool`, `std::sync::Arc`) were used ONLY by
  the macro body (now self-qualified) and by nothing else in the file. Removing them
  avoids an `unused_imports` error; the file's only remaining code is the invocation
  + list. Confirmed: ziee builds with zero new warnings (`-D unused-imports` clean).

- **DRIFT-1.4** — verdict: none. `#[cfg(not(test))]` semantics preserved. The
  `init_repositories` re-init warning is still gated on the INVOKING crate's `test`
  cfg — which is ziee, the same crate that formerly hosted the macro. No behavioral
  change (warning in non-test builds, silent overwrite in test builds).

- **DRIFT-1.5** — verdict: none. Emitted (caller-owned) item names left UNQUALIFIED
  deliberately — `$type::new`, `get_factory()`, `RepositoryFactory`, `Repos`, etc.
  are defined by the same expansion in ziee; qualifying them would be incorrect.
  Only the external-crate dependency paths were qualified. Confirmed by successful
  expansion (the wrappers Deref to `get_factory().$field()`; `Repos` builds).

- **DRIFT-1.6** — verdict: none. Wire surface unaffected. `Repos` is a
  process-internal repository accessor, never serialized/routed. E8 golden verified
  IDENTICAL on BOTH surfaces (ui + desktop): types.ts byte-identical, openapi.json
  canonically-equal (jq -S) — then restored via `git checkout`.

- **DRIFT-1.7** — verdict: none. ziee-desktop covered. It consumes `ziee`'s
  generated `Repos` (does not invoke the macro itself), so the single ziee-side
  expansion serves both binaries. Confirmed: `cargo check -p ziee-desktop` exit 0.

- **DRIFT-1.8** — verdict: none. Doc-comment doctest fence on the generated `Repos`
  const kept verbatim (plain ` ``` `, not `ignore`), so the tokens the macro emits
  in ziee are byte-identical to pre-move. No new/changed doctest behavior in ziee.

**Unresolved drifts:** 0
