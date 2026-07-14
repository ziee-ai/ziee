# TEST_RESULTS

Backend-only diff (`src-app/server/**`). No frontend workspace touched → no `npm run check` /
`gate:ui` / e2e gates apply. No new permission (no A9/A10). No new built-in MCP server (no A8).

Full logs: `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/fix-mcp-resource-link-rehost-int.log`
(integration) + `scratchpad/unit-rl.log` + `unit-guidance2.log` (unit).

## Per-TEST results

- **TEST-1**: PASS — `mcp::resource_link::tests::trusted_hosts_excludes_builtin_loopback_includes_admin_system` (unit; 15/15 in module).
- **TEST-2**: PASS — `mcp::resource_link::tests::choose_fetch_policy_trusts_registered_docker_host` (unit).
- **TEST-3**: PASS — `mcp::resource_link_test::system_server_host_is_trusted_and_result_file_ingested` (integration; ingest saved.len()==1, size==12, file_id stamped; negative control host rejected).
- **TEST-4**: PASS — `mcp::resource_link_test::accessor_returns_system_host_and_omits_builtin` (integration; redaction-bypass host present, dropped after is_built_in flip; shared-helper glue: external→host, built-in→empty).
- **TEST-5**: PASS — `mcp::chat_extension::mcp::tests::guidance_adds_file_url_rule_only_when_get_resource_link_present` + `saved_artifact_guidance_is_transient_and_steers_refetch` (unit).
- **TEST-6**: PASS — `code_sandbox::handlers::tests::get_resource_link_description_marks_url_transient_and_requires_refetch` (unit).

## Suite summaries
- Unit (`cargo test --lib -p ziee mcp::resource_link::`): **15 passed, 0 failed**.
- Unit (guidance/description filters): **5 passed, 0 failed** (incl. the pre-existing
  `guidance_always_includes_tool_preference_nudge` / `guidance_marks_resource_link_urls_short_lived...`
  regression guards — all still green).
- Integration (`cargo test --test integration_tests mcp::resource_link -- --test-threads=1`):
  **13 passed, 0 failed** — my 2 new tests + all pre-existing resource_link tests
  (`persist_ingests_ziee_under_root_and_handles_mixed_links`, `http_link_matched_trusted_host_is_ingested`,
  `http_link_unmatched_host_is_rejected`, `http_link_scoped_path_does_not_follow_redirect`,
  `refetch_reingest_yields_current_file_not_stale`, the ziee:// suite) — no regression.

Note: the server binary the harness spawns was rebuilt from this branch's code
(`cargo build -p ziee --bin ziee` into the per-worktree target, reached via the
`src-app/server/target` symlink — candidate #3; `src-app/target` doesn't exist here, so no
stale-binary shadowing).
