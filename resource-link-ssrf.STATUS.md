# resource-link-ssrf — worker status: DONE (PR open)

PR: https://github.com/ziee-ai/ziee/pull/131  (base: khoi, head: feat/resource-link-ssrf)
Worktree: /data/khoi/home-workspace/ziee/tmp/resource-link-ssrf-wt

- [x] Phases 1-9 lifecycle (8/8 gated + human-feedback ledger); blind audit converged to 0 findings
- [x] Fix: same-host trust (MCP_USER, redirects off) + release opt-in ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1
      for resource_link ingest; built-in/system (loopback) servers excluded from the trust set; IMDS blocked
- [x] Tests: unit 14/14, integration 10/10, workflow regression 2/2 — all green
- [x] .lifecycle stripped in the tip; commits authored khoi <khoi@tinnguyen-lab.com>, no AI attribution
- [ ] Live RCPA/DSCC repro on the user's stack (deferred to reviewer — needs the live containers/model)

Audit surfaced + fixed: a loopback-SSRF hole (built-in loopback host leaking into trusted_hosts) and
a false-green redirect test — both fixed and re-verified. Not merged (human to review/merge).
