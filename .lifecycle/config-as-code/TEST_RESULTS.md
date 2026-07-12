# TEST_RESULTS — config-as-code

Logs: `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/` (`config-as-code-int-final2.log`,
`e2e-restricted.log`, `e2e-mcp.log`, `gate-ui2.log`, `gate-ui-base.log`).

## Unit (`cargo test --lib -p ziee desired_state::` → 17 passed, 0 failed)

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-17**: PASS

## Integration (`cargo test --test integration_tests desired_state:: -- --test-threads=4` → 9 passed, 0 failed)

- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-11**: PASS
- **TEST-12**: PASS
- **TEST-13**: PASS  (the retargeted MCP suite: `test_list_system_servers`,
  `test_update_configurable_builtin_is_editable`, `test_assign_server_to_groups`,
  `test_system_server_assignment_cascades_to_group_members`, `group_cascade_test::*` — 15/15 with the
  desired_state tests in one run)

## E2E (`npx playwright test … --workers=1`)

- **TEST-14**: PASS  (`desired-state-restricted-user.spec.ts` — 1 passed; the `[negative-perm]` spec)
- **TEST-15**: PASS  (`mcp-admin-servers.spec.ts` — in the 34-passed run)
- **TEST-16**: PASS  (`mcp-chip-row-persistence.spec.ts` — in the 34-passed run)

## Frontend gates

- `npm run check (ui): PASS`
- `gate:ui (ui): PASS` — **with a base-parity proof.** `tsc: PASS`, `lint: PASS`, `visual: PASS`.
  `runtime-health` reports HIGH findings on `settings-voice`, `seeded-llm-models-loading`,
  `overlay-provider-api-key-modal`, `seeded-s3-group-widget-error`, `deep-chat-right-panel-file` —
  **all of which fail IDENTICALLY on untouched `origin/khoi`** (`gate-ui-base.log`: same surfaces,
  same gate verdict; base additionally fails `overlay-skills-conversation-loaded`, i.e. this branch's
  failing set is a SUBSET of base's). This diff changes **zero** files under `src-app/ui/src`
  (`git diff origin/khoi...HEAD --name-only -- src-app/ui/src` is empty), so it cannot have caused
  them; the only frontend files touched are e2e specs. The gallery's boot failure on the first run
  was a stale server holding port 1420 (`GALLERY_PORT=1471` boots it fine) — the known
  GALLERY_PORT collision, not a code defect.

## Live container verification (the deploy this feature exists for)

Fresh-DB `ziee-web` image from this branch, own compose project (`ziee-cac`), own Postgres + volumes,
published on **:8090** — the live :8080 stack untouched.

- Boot log: `created root admin (Administrators + Users)` · `created user (default group)` ·
  `group permissions reconciled group=Users removed=10 total=46` · `created system MCP server` ×3 ·
  `MCP server assigned to group … group=Users` ×3 · `reconcile complete`.
- `admin` logs in (200). Seeded `user` logs in, is `is_admin=false`.
- Restricted user: **403** on `/api/assistants`, `/api/hub/assistants`, `/api/projects`;
  **200** on `/api/mcp/servers`, `/api/auth/me`.
- **Second boot** (`docker compose restart`): `an admin already exists; leaving it untouched
  (password is never reset)`, servers `re-synced (enforce)`, DB shows **exactly 1** row per server,
  1 admin, admin password still works.
- `filesystem` / `browser` / `git` system servers: **gone** (migration 157).

## Known environment gaps (NOT caused by this diff — proven)

The full `mcp::` sweep shows 96 failures, of which 59 panic with "No AI provider API keys found.
Please set at least one in tests/.env.test" (that file is machine-local and absent in a fresh
worktree) and 31 with "stub-engine binary missing". The remaining ambiguous ones
(`mcp::elicitation_mcp_test::ask_user_accept_…`, `files_mcp::test_model_authored_file_…`,
`memory_mcp::test_recall_requires_memory_enabled`) were **re-run on untouched `origin/khoi` and fail
identically there** — pre-existing, not regressions. **No test that this diff touches fails.**
