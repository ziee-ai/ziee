# Chunk B3 — TESTS-MOVED

- **T-with_permission_documents_403_bearer_and_permission** [ported→sdk] file: `sdk/crates/ziee-framework/src/permissions/openapi.rs` covers: `with_permission` (403 + bearerAuth + permission-in-description decoration). Moved verbatim with the fn; the local `TestPerm` now `impl`s `ziee_identity::PermissionCheck`.

- **T-check_permission_union-suite** [stays→ziee] file: `src-app/server/src/modules/permissions/checker.rs` covers: `check_permission_union` (user-only, group-only, union, wildcard `*`, resource `foo::*`, hierarchical/deeply-nested wildcards, inactive-group-ignored, multiple-groups, no-permissions, large-set exhaustion). STAYS: `check_permission_union` is the CONCRETE union over ziee's `User`/`Group` and is not part of the enforcement move (still used by 5 other modules). The generic evaluator it delegates to (`check_permissions_array`) already carries its own tests in `ziee-identity` (B1b).

No covering test id present in any older committed `TESTS-MOVED.md` is absent now
(A5 shrink-guard: the only enforcement-path test, `with_permission_*`, is ported;
no extractor unit tests existed pre-move, so none were dropped).
