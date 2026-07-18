# Chunk B4 — `declare_repositories!` macro (CUT manifest)

Mechanical move. The `declare_repositories!` `macro_rules!` + ALL its generic
factory machinery (the `RepositoryFactory` struct/impl, the leaked-`&'static`
`FACTORY` RwLock, `init_repositories` / `get_factory` / `is_repos_initialized`,
the per-type `Deref` wrapper structs, `ReposAccessor`, and the `Repos` global
const — all EMITTED BY the macro) move from ziee's
`src-app/server/src/core/repository.rs` (macro body, former lines 26-190) into
`sdk/crates/ziee-framework` as a new `repository` module.

The concrete repo LIST — the `declare_repositories! { user: UserRepository =>
crate::modules::user, ... }` invocation over ziee's 33 module repo types (former
lines 198-233) — STAYS in ziee, now resolving the macro from `ziee_framework`.
The macro's expansion still generates `Repos` + all accessors IN ziee, so the
~171 `Repos.xxx` call sites are byte-for-byte unchanged.

## Files

- new:  `sdk/crates/ziee-framework/src/repository.rs` (the `#[macro_export]` macro)
- edit: `sdk/crates/ziee-framework/src/lib.rs` (`pub mod repository;`)
- edit: `src-app/server/src/core/repository.rs` (RETAINED: drops the macro def +
  the 3 now-unused `use` lines; adds `use ziee_framework::declare_repositories;`;
  keeps the `declare_repositories! { <the 33-entry list> }` invocation verbatim)

## Symbols

Macro (byte-preserved body except path-qualification — see TRANSFORMS):
- symbol: `declare_repositories!` (`sdk/crates/ziee-framework/src/repository.rs`)
  — `#[macro_export]`, so callable as `ziee_framework::declare_repositories!`.

Symbols the macro EMITS in the invoking crate (ziee) — unchanged surface:
`RepositoryFactory`, `init_repositories`, `is_repos_initialized`, `get_factory`
(private), `FACTORY` (private), `<Type>Repos` wrapper structs, `ReposAccessor`,
`Repos`. These are NOT defined in the framework — they materialize at the ziee
invocation site, exactly as before.

## Symbols that STAY in ziee (app-side — never moved)

- The `declare_repositories! { … }` LIST (33 `field: RepoType => module_path`
  entries) — ziee's concrete module wiring; naming `crate::modules::*` types the
  framework must never know.
- All ~171 `Repos.xxx` call sites across ziee modules — unchanged.
- ziee's `core::mod.rs` re-export `pub use repository::{Repos, init_repositories,
  is_repos_initialized};` — unchanged (the symbols are still generated in
  `core::repository`, so the re-export path is identical).

## Shim (retained ziee file)

`src-app/server/src/core/repository.rs` is retained (not deleted): it now holds
only the macro invocation + list. `use ziee_framework::declare_repositories;`
brings the `#[macro_export]` macro into scope so the existing invocation resolves.

## Design-gate

None — B4 is mechanical (no new abstraction/seam). The macro is already generic
over its `(field, type, module_path)` list; the only change is WHERE it is
defined + fully-qualifying the crate paths its expansion relies on so it works
from an external invoking crate. `Repos` stays app-side (generated at the ziee
invocation), so no framework-facing global is introduced.
