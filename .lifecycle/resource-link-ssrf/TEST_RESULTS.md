# TEST_RESULTS — resource-link SSRF fix

Backend-only diff (`src-app/server/**`) → no frontend workspace touched → no `npm run check` /
e2e / gate:ui gates apply. Ran the module's unit + integration tiers.

## Commands

```
# unit (14 passed; 0 failed) — src/modules/mcp/resource_link.rs #[cfg(test)]
cargo test -p ziee --lib mcp::resource_link::
# integration (10 passed; 0 failed) — tests/mcp/resource_link_test.rs, --test-threads=1
cargo test --test integration_tests mcp::resource_link_test:: -- --test-threads=1
# regression on the touched workflow dispatch path (2 passed; 0 failed)
cargo test --test integration_tests workflow::tool_step::tool_step_resource_link -- --test-threads=1
```

Logs: `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/resource-link-ssrf-{unit,int,workflow}.log`.

## Results (every enumerated TEST-ID)

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-11**: PASS

## Notes

- Unit tier (`mcp::resource_link::`, 14 passed) covers TEST-1..5, TEST-8, TEST-10, TEST-11 (plus 4
  pre-existing resource_link unit tests).
- Integration tier (`mcp::resource_link_test::`, 10 passed) covers TEST-6/7/9
  (`http_link_matched_trusted_host_is_ingested`, `http_link_unmatched_host_is_rejected`,
  `http_link_scoped_path_does_not_follow_redirect`) plus the 7 pre-existing resource_link tests —
  all green, so the new `trusted_hosts` parameter did not regress any existing path.
- Workflow regression: `tool_step_resource_link_is_saved_{false,true}` PASS (confirms the
  `dispatch.rs` call-site change is sound). Their first run failed only because the shared
  `stub-engine` fixture binary wasn't built in this fresh worktree (broken `src-app/target`
  build symlink); after building `stub-engine` they pass — an environment artifact, not a code defect.
