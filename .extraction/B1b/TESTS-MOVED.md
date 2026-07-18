# Chunk B1b — TESTS-MOVED (coverage-preservation)

Every ziee test that covered the moved code, and where it now runs. Ported tests
PASS in `cargo test -p ziee-identity` (22 passed, 0 failed); retained tests stay in
ziee and continue to exercise the concrete wrapper.

Permission trait/DTO tests (moved verbatim WITH the traits from `types.rs`):

- **T-B1b-1** [ported→sdk] file: `sdk/crates/ziee-identity/src/permission.rs` covers: PermissionCheck::to_info projection + JSON shape (to_info_projects_all_fields, to_info_serializes_to_expected_json_shape)
- **T-B1b-2** [ported→sdk] file: `sdk/crates/ziee-identity/src/permission.rs` covers: resource()/action() segment split incl. namespaced actions (resource_is_first_segment_action_is_last)
- **T-B1b-3** [ported→sdk] file: `sdk/crates/ziee-identity/src/permission.rs` covers: PermissionList single/multi format_description branches (format_description_single_permission, format_description_multiple_permissions, multi_permission_format_lists_all_with_descriptions, single_permission_format_uses_singular_header, multi_permission_format_description_lists_all_under_all_header)
- **T-B1b-4** [ported→sdk] file: `sdk/crates/ziee-identity/src/permission.rs` covers: PermissionList 3-/4-tuple ordered collection (permission_list_three_tuple_collects_all, permission_list_four_tuple_collects_all, three_tuple_yields_all_three_permissions_in_order, four_tuple_yields_all_four_permissions_in_order)

Generic RBAC evaluator (the moved `check_permissions_array` body):

- **T-B1b-5** [ported→sdk] file: `sdk/crates/ziee-identity/src/rbac.rs` covers: check_permissions_array directly — exact_match, full_wildcard_matches_anything, resource_wildcard, hierarchical_wildcard_arbitrary_depth, empty_set_denies (new direct-boundary coverage of the moved symbol)
- **T-B1b-6** [stays→ziee] file: `src-app/server/src/modules/permissions/checker.rs` covers: check_permission_union over concrete User/Group (all 14 tests — user/group/union, wildcard user+group, resource-wildcard, hierarchical, inactive-group-ignored, multiple-groups, no-perms, large-set exact+miss, buried-hierarchical, deeply-nested 4+ levels, large user+group sets). These retain in ziee because they exercise the CONCRETE wrapper (User/Group), which stays; they now drive the moved evaluator through the retained delegation, so the moved body is covered on both sides of the boundary.

New-interface tests (no prior ziee form):

- **T-B1b-7** [ported→sdk] file: `sdk/crates/ziee-identity/src/principal.rs` covers: Principal::has_permission UNION over direct+active-group, wildcard via group, and is_admin NOT folded into has_permission (union_of_direct_and_group_permissions, wildcard_via_group, is_admin_is_not_folded_into_has_permission)
