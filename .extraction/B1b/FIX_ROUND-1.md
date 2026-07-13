# Chunk B1b — FIX round 1

Findings from the C-3 blind audit (`LEDGER.jsonl`), triaged:

- One issue was resolved **during** the move loop (before this round) and is
  recorded as the reason for T-3: `ziee-identity` needed `schemars`/`serde` deps to
  carry `PermissionInfo`'s `JsonSchema`/serde derives once the type moved. Verified
  E8-neutral because `PermissionInfo` is not registered into the OpenAPI spec (0
  baseline occurrences), so `types.ts` stays byte-identical.

- The `Principal`-on-`User` group dimension (empty active-group set today) was
  reviewed and confirmed **not a defect**: authorization is threaded through
  `check_permission_union(user, groups, ..)` at call sites and `User` alone does not
  carry its groups; nothing calls `User::has_permission` this chunk (the extractor
  stays until B3), so the live authorization path is byte-for-byte unchanged. Wiring
  the group dimension into `Principal` is B3 scope, documented in the TRANSFORMS
  `## Decision`.

- All other ledger entries are `status: ok` (verified-equivalent) — the trait/DTO
  and RBAC-eval bodies are verbatim, delegation is call-for-call identical, both
  clean-builds are green, and the golden anchors hold (types.ts byte-identical,
  openapi.json canonically-equal).

No new B1b-introduced defects were confirmed by the audit.

**New confirmed findings:** 0
