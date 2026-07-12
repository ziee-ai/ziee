# FIX_ROUND-1 — fixes applied after the phase-6 blind audit

Eight blind agents (12 angles) reviewed the diff with no access to my reasoning.
21 findings landed in `LEDGER.jsonl`; 18 confirmed and fixed, 3 rejected with rationale.

## The ones that mattered

1. **The feature did not work at all (HIGH).** `Repos.mcp.assign_to_group`'s parameters are NAMED
   `(server_id, group_id)` but forward positionally into
   `assign_mcp_server_to_group(pool, group_id, server_id)`. The three pre-existing callers silently
   compensated by passing them swapped; my call trusted the names, so EVERY group assignment failed
   with `not_found("Server")` (soft-logged) and the org MCP servers were created but never made
   available to the Users group — i.e. unusable by exactly the non-admin users this feature exists
   for. Fixed by renaming the wrapper's params to the truth (a pure rename: forwarding is unchanged
   and all pre-existing callers become correct-by-name) and fixing the call site.
2. **The boot health check would silently disable the org servers in production (HIGH).**
   `mcp::init` spawns `run_startup_health_check`, which probes every enabled non-built-in server and
   AUTO-DISABLES the unreachable ones. A declared server whose endpoint lags the deploy gets
   `enabled = false`, and `ensure` mode would never re-enable it. The shipped manifest now declares
   the servers `mode: enforce` (each deploy re-asserts `enabled`), documented in the module, the
   manifest and the README.
3. **The real deploy path was inert (HIGH).** `docker-compose.external-db.yml` — the file whose own
   header says it is "the shape a TeamCity-deployed environment uses" — did not pass the five new
   env vars, so on that path no servers, no admin and no user would have been created.
4. **Fail-open on a broken manifest (HIGH).** A bind-mount of a missing host path makes Docker
   create a DIRECTORY; the read failed, was logged, and the container served anyway — with no admin
   (leaving the unauthenticated first-run setup page open) and no permission trim. A bad FILE is now
   fatal; a bad ENTRY is still skipped. Added regular-file + size guards.
5. **Concurrent boots could duplicate the servers (MED).** `mcp_servers.name` has no unique index, so
   the check-then-insert raced. The whole reconcile now runs under a Postgres advisory lock.
6. Plus: `ensure` mode never converged a failed group assignment; `add:` accepted a `*` wildcard
   (privilege escalation from a typo'd manifest); the docs advertised a `mode` key on `admin`/`users`
   that `deny_unknown_fields` would have rejected — failing the WHOLE file; migration 157 could
   delete an operator's own system server named `git`; the unit tests used `unsafe std::env::set_var`
   (a real data race against other threads of the same test binary); three README claims were false.

## Rejected (with rationale, in the ledger)

- `password: ${HOME}` passes the inline-secret guard — the manifest is trusted deploy config at the
  same level as `config.yaml`; an actor who can edit it can already set any password.
- The gallery cassette still lists the deleted servers — recorded mock data, no gate reads it.
- `reboot()`'s port TOCTOU — it is the same scheme the shared harness itself uses.

**New confirmed findings:** 18
