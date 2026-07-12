# FIX_ROUND-4 — fourth blind round + the lead's deploy update

The round-4 blind agent independently re-verified the load-bearing fixes (the `assign_to_group`
rename across all five call sites incl. the two desktop ones; `starts_with` availability on
`pgvector/pgvector:pg18`; the removal of the dead e2e fixture; the `add:`-rejection logging) and
found exactly ONE real defect, now fixed:

1. **The e2e fixture treated an EXACT `remove:` entry as a wildcard (LOW).** `toPrefix()` passed a
   non-`::*` pattern through unchanged, and the SQL then applied BOTH the equality AND the
   `starts_with(p, pre || '::')` clause to it — so a future manifest entry like `remove: [hub::models]`
   would have stripped `hub::models::read` in the spec while the real reconciler strips only the exact
   string, making the spec assert a UI state production never produces. Now the patterns are split
   into wildcard-prefixes and exact matches and each is applied with the reconciler's own semantics.
   Re-ran the spec: PASS.

## Deploy info folded in (from the lead, mid-round — not an audit finding)

The three MCP servers are reached over `host.docker.internal` at remapped ports (the deploy host only
allows 18000-19000), and their row names are `rcpa-user` / `dscc-user` / `biognosia-user`:

- `config/desired-state.yaml`: server names updated; the deploy URLs documented as the values for the
  (still env-templated) `${RCPA_MCP_URL}` / `${DSCC_MCP_URL}` / `${BIOGNOSIA_MCP_URL}` — nothing is
  hardcoded, so local dev keeps pointing them at `172.21.0.1:9004` etc.
- **Required compose dependency**: `extra_hosts: ["host.docker.internal:host-gateway"]` added to BOTH
  `docker-compose.yml` and `docker-compose.external-db.yml`, and documented in `docker/web/README.md`
  (with the `--add-host` equivalent for a hand-rolled `docker run`).
- Unit + integration tests follow the new names and the deploy-shaped URLs: 17/17 unit, 9/9 integration.

**New confirmed findings:** 1
