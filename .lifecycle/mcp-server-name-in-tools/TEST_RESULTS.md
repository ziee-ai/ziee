# TEST_RESULTS

Backend-only diff (`src-app/server/**`) → backend chain only. No `src-app/ui/**`
touched, no OpenAPI/types regen, no new permission → no frontend gate, no e2e, no
`[negative-perm]` spec required. Full logs:
`/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/mcp-server-name-in-tools-{unit,int}.log`.

Run against the final committed HEAD:
`source tests/.env.test && cargo test --test integration_tests -- --test-threads=1 <names>`
and `cargo test --lib -p ziee -- <names>`.

- **TEST-1**: PASS  (`convert_mcp_tool_labels_description_not_name` — unit)
- **TEST-2**: PASS  (`convert_mcp_tool_label_edge_cases` — unit)
- **TEST-3**: PASS  (`connected_servers_section_renders_roster` — unit)
- **TEST-4**: PASS  (`external_tools_labeled_and_rostered_builtins_untouched` — integration)
- **TEST-5**: PASS  (`labeled_external_tool_still_dispatches` — integration)
- **TEST-6**: PASS  (`sanitize_prompt_field_collapses_and_caps` — unit)

Unit: `test result: ok. 4 passed; 0 failed`. Integration: `test result: ok. 2 passed; 0 failed`.

Negative-controlled (per ziee-negative-control-your-tests): reverting the label +
roster turned TEST-1/2/3 red (0 passed; 3 failed) with the intended diagnostics;
restored clean and all pass again.
