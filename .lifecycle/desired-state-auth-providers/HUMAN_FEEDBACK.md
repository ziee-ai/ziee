# HUMAN_FEEDBACK

**No human feedback received** — this feature was implemented autonomously per the
task file `ziee-worker-tasks/desired-state-auth-providers.md`; no human has yet
reviewed the running feature. The ledger is ready to record any feedback verbatim
when it arrives.

## Notes for the reviewer (surfaced proactively, not gating)

- **Incidental fix (DRIFT-1.2):** the pre-existing unit test
  `shipped_desired_state_file_is_valid` was already RED on `khoi` — it asserted the
  stale MCP server names `rcpa-user`/`dscc-user`/`biognosia-user`, but commit
  e597a99d8 renamed the shipped `config/desired-state.yaml` entries to
  `rcpa`/`dscc`/`biognosia` without updating this test. Since this feature extends
  exactly that test (to assert the new `google` entry), the stale names were
  aligned to the shipped file (a one-line, trivially-correct fix). Called out here
  so it isn't mistaken for scope creep.
- **Deploy action required:** register the redirect URI
  `<public-origin>/api/auth/oauth/google/callback` on the Google Cloud OAuth
  production client, and ensure the ingress forwards `X-Forwarded-Proto: https` +
  the real public `Host` (documented in DEPLOY.md and docker/web/README.md).
