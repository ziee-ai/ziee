# Chunk B4 — TESTS-MOVED

No tests moved and none dropped: the pre-extraction ziee `core/repository.rs`
contained NO `#[cfg(test)]` block — the macro had no dedicated unit tests. The
`declare_repositories!` expansion is exercised transitively by the entire
integration suite (every test that touches `Repos.*` / `init_repositories` /
`is_repos_initialized`), which is unchanged by this move (the generated symbols
land in the same `ziee::core::repository` module).

- **T-none** [n/a] — no covering test id existed for the macro pre-move, so none
  is ported and none is removed.

A5 shrink-guard: no covering test id present in any older committed
`TESTS-MOVED.md` (chunk0..B3) named `declare_repositories!` / `Repos` /
`RepositoryFactory`, so nothing is dropped or renumbered-away here.
