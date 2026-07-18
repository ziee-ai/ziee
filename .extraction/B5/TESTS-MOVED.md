# Chunk B5 — TESTS-MOVED

## Moved into `ziee-framework` (16 unit tests)

From `ziee` `sync/registry.rs` → `crates/ziee-framework/src/sync/registry.rs`
(re-expressed over a framework-side `TestPrincipal: Principal` + `TestEntity:
SyncEntityKind` + dummy `axum::Event`s — the routing/pruning logic never inspects
the wire payload, so the entity-agnostic form is a *stricter* unit of the routing
core; ziee's wire-format tests stay app-side, below):

- `owner_audience_isolates_users`
- `origin_connection_is_skipped_but_other_tabs_are_not`
- `permission_audience_excludes_non_holders_includes_holders_and_admins`
- `group_scoped_audience_routes_by_group_membership` (exercises the
  `Principal::active_group_permissions` path — group-derived perm routing)
- `everyone_audience_reaches_all_connections`
- `per_user_cap_rejects_excess_connections`
- `rapid_fire_deliveries_are_all_enqueued_in_order`
- `global_cap_rejects_excess_connections_across_users` (429)
- `unregister_cleans_up_indexes`
- `refresh_updates_permission_snapshot`
- `deliver_session_to_users_targets_only_listed_users_and_skips_origin`
- `deliver_session_to_users_prunes_a_lagging_connection` (audit id 97e64997158)
- `lagging_connection_is_pruned`
- `has_permission_uses_the_shared_evaluator` (NEW — pins that
  `ziee_identity::check_permissions_array` backs `Perm` routing; justifies the import)

From `ziee` `sync/event.rs` → `crates/ziee-framework/src/sync/audience.rs`:

- `perm_constructor_carries_the_typed_permission_string`
- `all_of_and_any_of_collect_the_permission_tuple`

Result: `cargo test -p ziee-framework --lib sync::` → **16 passed; 0 failed**.

## Stayed in `ziee` (schema/union coverage — deliberately NOT moved)

- `sync/event.rs`: `wire_payload_is_notify_only_snake_case`,
  `entity_names_match_the_frontend_sync_vocabulary`,
  `kb_wire_tests::kb_entities_serialize_snake_case` — these pin the CONCRETE,
  schema-bearing wire types (the `sync:<entity>` frontend vocabulary `types.ts`
  depends on), which did not move.
- `permissions/checker.rs`: the full `check_permission_union` union/wildcard/
  inactive-group suite — the concrete union semantics `SyncConnPrincipal::Principal`
  reproduces; unchanged. `ziee_identity`'s own `check_permissions_array` +
  `Principal` tests cover the generic evaluator on the framework side.

## No tests deleted; no `#[ignore]` added.

Golden (both surfaces) is the integration-level equivalence gate for this chunk
(no new backend integration test needed — the SSE subscribe integration test in
`server/tests/sync/subscribe_test.rs` exercises the app-side path unchanged).
