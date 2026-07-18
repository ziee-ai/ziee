# Chunk B4 — TRANSFORMS (every non-byte-identical change + rationale)

The macro body is byte-identical to its pre-extraction ziee form EXCEPT the
path-qualification transform (T-1). The `declare_repositories! { … }` invocation
+ its 33-entry list are verbatim. The doc-comment example on the generated `Repos`
const is verbatim (including the plain ` ``` ` doctest fence — kept unchanged so
the tokens the macro emits in ziee are identical to before).

- **T-1** `declare_repositories!` macro body (`ziee-framework::repository`): every
  crate path the EXPANSION relies on is rewritten from an unqualified/prelude-relative
  form to a fully-qualified `::crate` form, so the expansion resolves in the invoking
  crate regardless of that crate's `use` imports:
  - `OnceCell` → `::once_cell::sync::OnceCell` (was `use once_cell::sync::OnceCell;`
    at the top of ziee's file)
  - `PgPool` → `::sqlx::PgPool` (was `use sqlx::PgPool;`)
  - `Arc` → `::std::sync::Arc`; `Arc::new` → `::std::sync::Arc::new` (was
    `use std::sync::Arc;`)
  - `std::sync::RwLock` → `::std::sync::RwLock`; `std::ops::Deref` → `::std::ops::Deref`
    (leading `::` added — already `std::`-qualified, now absolute)
  - `paste::paste!` → `::paste::paste!`
  - `tracing::warn!` → `::tracing::warn!`
  — **why:** a `#[macro_export]` macro invoked from another crate resolves its
  hard-coded (non-`$`-substituted) paths in the INVOKING crate's namespace. The
  three `use` lines that formerly provided `OnceCell`/`PgPool`/`Arc` lived in ziee's
  `repository.rs` and are DELETED (T-3); fully-qualifying the paths makes the macro
  self-contained. ziee already depends on `once_cell`/`sqlx`/`paste`/`tracing`/`std`,
  so every `::crate` name is in its extern prelude. Tokens that come FROM the
  invocation (`$module_path`, `$type`, `$field`) are untouched — their `crate::`
  prefixes still resolve against ziee, exactly as when the macro lived there.
  `Box::leak`/`Box::new` use the always-available `Box` prelude item (left as-is,
  matching the original). `#[cfg(not(test))]` inside the macro still keys off the
  INVOKING crate's `test` cfg (ziee) — same crate as before, so identical behavior.

- **T-2** `declare_repositories!` gains `#[macro_export]` (`ziee-framework::repository`).
  — **why:** exports the macro at the framework crate root so ziee can name it
  (`ziee_framework::declare_repositories!` / `use ziee_framework::declare_repositories;`).
  The former in-crate macro needed no export (textual scope). No behavioral effect
  on the expansion.

- **T-3** ziee `core/repository.rs`: the macro definition (former lines 26-190) is
  DELETED; the three `use once_cell::sync::OnceCell; use sqlx::PgPool; use std::sync::Arc;`
  lines (former 10-12) are DELETED; a `use ziee_framework::declare_repositories;` +
  a 5-line provenance comment are ADDED above the retained invocation.
  — **why:** N2 surface preservation. The `use` imports are now dead (the macro
  self-qualifies via T-1, and nothing else in the file references them), so keeping
  them would trip `-D unused-imports`. The invocation + list (former 198-233) and
  the file's top-of-file doc banner are unchanged.

- **T-4** ziee-framework `lib.rs`: `pub mod repository;` added (with a provenance
  comment). — **why:** compile the new macro file. No `pub use` re-export is needed
  — `#[macro_export]` already publishes the macro at the crate root, and ziee imports
  it by that path.

## Decision — fully-qualified paths vs `$crate` vs re-exported prelude

A `#[macro_export]` macro invoked cross-crate must resolve its hard-coded paths in
the caller's namespace. Three options: (a) fully-qualify each to `::crate::…`;
(b) route each through `$crate::…` (i.e. re-export `once_cell`/`sqlx`/`paste`/
`tracing`/`Arc` from `ziee_framework` and reference them as `$crate::…`); (c) rely
on the caller having the right `use` imports in scope (the pre-move status quo,
which only worked because the macro was in-crate).

**Resolution:** option (a). It is the minimal, dependency-explicit change — the
invoking crate (ziee) already depends on all five crates, so `::once_cell` /
`::sqlx` / `::std` / `::paste` / `::tracing` are guaranteed in its extern prelude.
Option (b) would force `ziee-framework` to add + re-export those crates purely as
macro plumbing (extra deps, extra surface) with no other consumer; the macro is
pure tokens (not type-checked at the framework, so it needs no deps of its own).
Option (c) is rejected — it re-couples correctness to the caller's imports and
would break any future non-ziee invoker. `$type::new` / the emitted item names
(`RepositoryFactory`, `<Type>Repos`, `ReposAccessor`, `Repos`, `init_repositories`,
etc.) stay UNQUALIFIED because they are defined by the same expansion in the caller
— fully-qualifying them would be wrong (they are the caller's items). Verified: the
macro expands + `Repos` builds in both ziee (lib+bin) and ziee-desktop.

## Decision — keep `Repos` generated app-side (not moved to framework)

The task + plan require the `Repos` global + accessors to stay in ziee. The macro
emits them at the invocation site; since the invocation stays in ziee's
`core::repository`, the generated symbols land in `ziee::core::repository` exactly
as before.

**Resolution:** move ONLY the macro tokens; leave the invocation + list in ziee.
`core::mod.rs`'s `pub use repository::{Repos, init_repositories, is_repos_initialized}`
is unchanged — the re-exported items are still generated in that module. No
framework-facing global is introduced (the framework holds a macro, never a
`Repos` instance). This is the whole point of a code-gen macro: the framework owns
the GENERATOR, the app owns the GENERATED singleton + its concrete type list.
