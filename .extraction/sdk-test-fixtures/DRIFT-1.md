# Chunk `sdk-test-fixtures` — DRIFT-1

**Drift count: 0.**

Blind re-audit of the full `git diff` (superproject src-app deletions + Cargo/mod
edits; sdk submodule: 4 fixture moves + lib.rs/Cargo.toml). Every changed hunk is
accounted for by CUT.md + TRANSFORMS.md (T-1..T-5):

- 3 fixtures + the `.p8` = git-recorded renames, 0 content delta.
- `sync_probe.rs` = the single T-1 signature line + its doc-comment.
- `fixtures/mod.rs` (new) = module decls + doc, no logic.
- `lib.rs` = T-2 (mod + trait) only.
- `Cargo.toml` (harness) = T-3 (optional deps + feature) only.
- `mod.rs` (ziee) = T-4 (shims + impl) only.
- `Cargo.toml` (ziee server) = T-5 (feature flag) only.

No unexplained hunk. No behavioural change beyond the intended genericity seam.
No `ziee::`/`ziee-chat`/secret leak in any moved file. Plan ↔ diff = fully
converged.
