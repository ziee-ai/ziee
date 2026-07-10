# DRIFT-1 — implementation vs plan

Reviewed the full `git diff origin/khoi...HEAD` against PLAN.md / TESTS.md / DECISIONS.md.

- **DRIFT-1.1** — verdict: impl-wins — The plan's ITEM-3/ITEM-4 originally inlined the trusted-host
  derivation (`.filter_map(host_of).collect()`) at each call site. The implementation extracted a
  shared `trusted_hosts_from_urls(urls)` helper (dedup + lowercase) used by all three call sites
  (DRY, and unit-testable). PLAN.md ITEM-1/ITEM-3/ITEM-4 were amended to name the helper; TEST-10
  was retargeted to a `tier: unit` test on the helper (TESTS.md updated). No behavior change — the
  helper produces the same host set. Resolved.
- **DRIFT-1.2** — verdict: none — ITEM-1/ITEM-2 (`host_of`, `choose_fetch_policy`,
  `fetch_policy_and_redirects`, `resource_link_allow_private_env`, external-branch rewrite,
  redirect-disabled scoped client) match the plan exactly, including the debug-seam-preserving
  precedence (DEC-7).
- **DRIFT-1.3** — verdict: none — ITEM-5 (7 test call sites threaded with the new `&[]`/fixture
  `trusted_hosts` arg; doc comments updated) and ITEM-6 (CLAUDE.md external-link SSRF note) match.
- **DRIFT-1.4** — verdict: resolved — A stray `src-app/server/target` build symlink (worktree
  build-env artifact) was accidentally `git add -A`'d; untracked via `git rm --cached` + a local
  `.git/info/exclude` entry, so the committed tree carries only the 5 intended source files.
- **DRIFT-1.5** — verdict: none — DEC-5 (system-server `url` redaction limitation) is honored: the
  call-site comments + CLAUDE.md both document that same-host *system*-registered servers fall back
  to the `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE` opt-in.

Both the lib unit-test target and the integration test target compile clean (only pre-existing
dead-code warnings unrelated to this diff).

**Unresolved drifts:** 0
