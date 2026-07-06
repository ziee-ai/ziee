# PLAN_AUDIT — project-search

Audit of PLAN.md against the origin/main codebase.

## Breakage risk

- `project::PaginationQuery` (project/handlers.rs:37) is used by **two**
  extractors: `list_projects` (handlers.rs:241) **and**
  `project/chat_extension/handlers.rs:34`. Adding an `Option<String> search`
  field is **additive and backwards-compatible**: existing callers omit it
  (deserializes to `None`), and the chat_extension endpoint will silently accept
  but ignore a `?search=` param. No caller breaks. (Recorded so the reviewer
  isn't surprised the field appears on a second endpoint's OpenAPI.)
- `project::Repository::list_for_user` (repository.rs:88) has exactly **one**
  caller — handlers.rs:246. (The other `list_for_user` hits in the tree belong
  to the memory/workflow/llm_provider repos — different types, out of scope.)
  Changing its signature touches that one call site only.
- SQL: the new predicate is added to **both** the page SELECT and the COUNT so
  `total` and `projects[]` stay consistent (the existing twin-query shape).

## Pattern conformance

- Search param mirrors the established convention: `mcp/handlers/user.rs`
  (`search: Option<String>` → `.as_deref().map(str::trim).filter(|s| !s.is_empty())`)
  and `mcp/repository.rs` (`name ILIKE '%' || $N || '%'`). ITEM-2/ITEM-3 replicate
  this exactly. Conforms.
- `PaginationQuery` field added to both the public struct and the private `Raw`
  in the custom `Deserialize` — matches the existing struct's own idiom. Conforms.

## Migration collisions

- **None.** This feature adds no migration (query-only change). `ls migrations/`
  tops out at `00000000000130_*`; nothing is added, so no collision is possible.

## OpenAPI regen

- **Required.** Adding `search` to `PaginationQuery` changes the generated query
  schema for `Project.list` (and the chat_extension list endpoint). The committed
  `openapi.json` + `types.ts` must be regenerated or the `types_ts_parity` golden
  lib test fails. This is ITEM-4's whole job. Regen via the documented server
  command (writes `ui/openapi/openapi.json` + `ui/src/api-client/types.ts`);
  run desktop regen too if `just openapi-regen` includes it.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — additive optional field; backwards-compatible on both extractors that use `PaginationQuery`.
- **ITEM-2** — verdict: PASS — direct mirror of the mcp trim-to-None handler idiom; single handler body edit.
- **ITEM-3** — verdict: PASS — parameterized `ILIKE` predicate applied to both SELECT and COUNT; single call site to update.
- **ITEM-4** — verdict: CONCERN — must actually run the regen or the parity golden test breaks; tracked explicitly and covered by TEST in Phase 3.
