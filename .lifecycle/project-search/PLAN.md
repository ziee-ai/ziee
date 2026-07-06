# PLAN — project-search

**Feature:** add an optional case-insensitive `search` filter to
`GET /api/projects`, so a user with many projects can find one by typing part of
its name (or description). Backend-only, no migration, one new query parameter.

**User-visible value:** `GET /api/projects?search=roadmap` returns only projects
whose name/description contains "roadmap" (paginated + total reflect the filter).
This is the API capability the projects list UI's search box consumes.

## Items

- **ITEM-1**: Add a schema-visible `search: Option<String>` field to the project `PaginationQuery` (struct + its custom `Deserialize` `Raw`) so OpenAPI marks it `required: false`.
- **ITEM-2**: Thread `search` from the `list_projects` handler into the repository call, normalizing blank/whitespace to `None` so `?search=` and `?search=%20` behave as "no filter".
- **ITEM-3**: Filter `list_for_user` by case-insensitive substring on `name` OR `description`, applied identically to both the page SELECT and the COUNT query, fully parameterized (no string interpolation).
- **ITEM-4**: Regenerate OpenAPI + TS types so the committed `openapi.json` / `types.ts` include the new `search` query parameter (keeps the `types_ts_parity` golden test green).

## Files to touch

- `src-app/server/src/modules/project/handlers.rs` — `PaginationQuery` struct, its `Deserialize`, and `list_projects` (ITEM-1, ITEM-2); plus a `#[cfg(test)]` unit test.
- `src-app/server/src/modules/project/repository.rs` — `list_for_user` signature + SQL (ITEM-3).
- `src-app/ui/openapi/openapi.json` + `src-app/ui/src/api-client/types.ts` — regenerated (ITEM-4). Desktop equivalents if `just openapi-regen` touches them.
- `src-app/server/tests/project/search_test.rs` — new integration test module (Phase 3).
- `src-app/server/tests/project/mod.rs` — register `mod search_test;`.

## Patterns to follow

- **Query-param search** → mirror `modules/mcp/repository.rs` (the
  `name ILIKE '%' || $N || '%'` predicate) and `modules/mcp/handlers/user.rs`
  (the `search: Option<String>` query field + trim-to-None handling). This is the
  established list-search convention in this codebase.
- **PaginationQuery custom `Deserialize`** → the existing project
  `handlers.rs::PaginationQuery` (add the field to both the public struct and the
  private `Raw`).
- **Repository list shape** → the existing `list_for_user` (keep the twin
  SELECT + COUNT structure; add the same predicate to both).
- **Integration test** → `tests/project/crud_test.rs` (`create_get_list_update_delete`
  list assertions) + `tests/project/helpers.rs` (`full_project_permissions`, user
  setup). One user, seeded projects, assert filtered `total` + `projects[]`.
