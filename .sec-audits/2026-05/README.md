# Server Security Audit — Fresh Round (2026-05-23)

## Overview

This directory contains the fresh-round security audit of the Ziee Chat server (`src-app/server/`), conducted on **2026-05-23** against **OWASP ASVS 4.0.3 Level 2**. Fourteen per-module audits were performed in parallel by general-purpose ASVS-aligned reviewers, covering ~58,000 lines of Rust source across the entire HTTP / DB / business-logic surface. The audit is **pure code review** — no source modifications, no `cargo`/`git`/`sqlx`/`docker`/`npm` commands executed, no dynamic testing.

The round identified **256 findings** (Critical: 11, High: 49, Medium: 85, Low: 65, Info: 46). The headline conclusion is that the codebase has **strong structural design** (typed permissions, compile-time-checked SQL, fail-closed extractors, PKCE-based OAuth) but **shipping-grade operational hardening is missing** across the board — body limits, rate limiting, security headers, secret encryption at rest, SSRF defences, error-message redaction, request timeouts. Roughly 60–70% of findings cluster around seven cross-cutting themes, all of which are documented in [`00-EXECUTIVE-SUMMARY.md`](./00-EXECUTIVE-SUMMARY.md).

**Intentionally excluded from this round:** the MCP module (audited separately, see `../05-mcp-module-audit.md` and `../mcp-phase3-i2-get-sse-audit-2026-05-22.md`); the code_sandbox module (audited separately, see `../wsl2-sandbox-prior-art-2026-05-22.md` and `../wsl2-source-deep-read-2026-05-22.md`); the frontend (`src-app/ui/`); dependency CVE scanning via `cargo audit`; dynamic / runtime exploit testing; reverse-proxy / TLS-termination assumptions.

---

## Audit Files (14)

Each per-module audit file uses the OWASP ASVS 4.0.3 format with `F-NN` findings, vulnerable code snippets, exploitation scenarios, impact analysis, and recommended fixes.

| # | File | Module(s) Audited | LOC | Critical | High | Medium | Low | Info | Total |
|---|---|---|---:|---:|---:|---:|---:|---:|
| 01 | [`01-auth.md`](./01-auth.md) | `modules/auth/` (JWT, OAuth2/OIDC, LDAP, login/signup) | 2,824 | 1 | 6 | 7 | 4 | 2 | 20 |
| 02 | [`02-permissions.md`](./02-permissions.md) | `modules/permissions/` + cross-module route-gating | 758 | 0 | 4 | 6 | 5 | 4 | 19 |
| 03 | [`03-user.md`](./03-user.md) | `modules/user/` (users + groups CRUD) | 2,026 | 2 | 4 | 6 | 5 | 4 | 21 |
| 04 | [`04-chat.md`](./04-chat.md) | `modules/chat/` (core + assistant/file/mcp/text/title extensions) | 11,012 | 1 | 3 | 7 | 5 | 3 | 19 |
| 05 | [`05-file.md`](./05-file.md) | `modules/file/` (upload, storage, OCR, processing, ACL) | 3,292 | 1 | 5 | 11 | 6 | 4 | 27 |
| 06 | [`06-llm-provider.md`](./06-llm-provider.md) | `modules/llm_provider/` + `modules/llm_provider_files/` | 2,195 | 2 | 4 | 7 | 5 | 4 | 22 |
| 07 | [`07-llm-model.md`](./07-llm-model.md) | `modules/llm_model/` + git/LFS service cross-boundary | 5,045 | 1 | 5 | 6 | 5 | 3 | 20 |
| 08 | [`08-llm-local-runtime.md`](./08-llm-local-runtime.md) | `modules/llm_local_runtime/` (engine binary fetch + spawn) | 2,951 | 0 | 4 | 5 | 5 | 2 | 16 |
| 09 | [`09-llm-repository.md`](./09-llm-repository.md) | `modules/llm_repository/` (HF/GitHub registry catalogue) | 1,169 | 1 | 5 | 6 | 4 | 1 | 17 |
| 10 | [`10-assistant.md`](./10-assistant.md) | `modules/assistant/` (user-owned + system-template assistants) | 1,644 | 0 | 1 | 3 | 4 | 1 | 9 |
| 11 | [`11-hub.md`](./11-hub.md) | `modules/hub/` (curated marketplace, embedded + GitHub-refresh) | 2,116 | 0 | 0 | 4 | 4 | 7 | 15 |
| 12 | [`12-hardware.md`](./12-hardware.md) | `modules/hardware/` (HW detection + SSE monitoring) | 1,529 | 0 | 2 | 3 | 3 | 3 | 11 |
| 13 | [`13-misc-small-modules.md`](./13-misc-small-modules.md) | `modules/app/`, `modules/health/`, `modules/onboarding/` | 649 | 0 | 2 | 4 | 5 | 4 | 15 |
| 14 | [`14-core-infrastructure.md`](./14-core-infrastructure.md) | `main.rs`, `lib.rs`, `core/`, `common/`, `module_api/`, `utils/`, `build.rs`, `Cargo.toml`, `config/` | ~3,000 | 2 | 4 | 10 | 5 | 4 | 25 |
| | **Totals** | | **~40,210** | **11** | **49** | **85** | **65** | **46** | **256** |

---

## Rollup Files (3)

- **[`00-EXECUTIVE-SUMMARY.md`](./00-EXECUTIVE-SUMMARY.md)** — Read first. Severity distribution, top 10 critical/high risks (prioritised), cross-cutting themes (the seven recurring root causes that drive ~60–70% of findings), ASVS coverage matrix, four-phase remediation plan, comparison vs. prior 2025-01 and 2025-11 rounds, and system-wide strengths to preserve through remediation.
- **[`REMEDIATION-CHECKLIST.md`](./REMEDIATION-CHECKLIST.md)** — One-row-per-finding, mechanical checklist of all 256 findings with checkbox, severity, ASVS reference, CWE, one-line title, and `file:line` actionable location. Severity-ordered, then by audit-file number and `F-NN`. Designed to be ticked off as remediation lands; preserved across PRs by version control.
- **[`README.md`](./README.md)** — This file. Index, scope, methodology, related-audits cross-reference, finding-anatomy guide, re-audit cadence recommendation.

---

## Related Audits (Outside This Round)

The parent directory `.sec-audits/` contains audits from earlier rounds and dedicated workstreams that are still authoritative for the modules they cover. Cross-reference when remediation work spans this round and an earlier one.

### Still authoritative (use these, not the 2025-11 versions)

- [`../05-mcp-module-audit.md`](../05-mcp-module-audit.md) (2025-01) — MCP module security baseline. The MCP module is **not** re-audited in the 2026-05 round.
- [`../mcp-phase3-i2-get-sse-audit-2026-05-22.md`](../mcp-phase3-i2-get-sse-audit-2026-05-22.md) — Standalone GET-SSE MCP client audit (Plan 3 Phase 3).
- [`../wsl2-sandbox-prior-art-2026-05-22.md`](../wsl2-sandbox-prior-art-2026-05-22.md) — code_sandbox WSL2 prior-art review.
- [`../wsl2-source-deep-read-2026-05-22.md`](../wsl2-source-deep-read-2026-05-22.md) — code_sandbox WSL2 source deep read.
- [`../plan-4-g7-g8-decision-2026-05-23.md`](../plan-4-g7-g8-decision-2026-05-23.md) — Plan 4 MCP G7/G8 decision rationale.

### Superseded by this round (consult 2026-05 files instead)

- `../01-auth-user-permissions-audit.md` (2025-01) → superseded by [`01-auth.md`](./01-auth.md), [`02-permissions.md`](./02-permissions.md), [`03-user.md`](./03-user.md).
- `../02-chat-module-audit.md` (2025-01) → superseded by [`04-chat.md`](./04-chat.md).
- `../03-file-module-audit.md` (2025-11) → superseded by [`05-file.md`](./05-file.md).
- `../04-llm-modules-audit.md` (2025-11) → superseded by [`06-llm-provider.md`](./06-llm-provider.md), [`07-llm-model.md`](./07-llm-model.md), [`08-llm-local-runtime.md`](./08-llm-local-runtime.md), [`09-llm-repository.md`](./09-llm-repository.md).
- `../06-assistant-hub-audit.md` (2025-01) → superseded by [`10-assistant.md`](./10-assistant.md), [`11-hub.md`](./11-hub.md).
- `../07-core-infrastructure-audit.md` (2025-11) → superseded by [`14-core-infrastructure.md`](./14-core-infrastructure.md).
- `../00-EXECUTIVE-SUMMARY.md` (2025-11) → superseded by [`00-EXECUTIVE-SUMMARY.md`](./00-EXECUTIVE-SUMMARY.md).
- `../REMEDIATION-CHECKLIST.md` (2025-11) → superseded by [`REMEDIATION-CHECKLIST.md`](./REMEDIATION-CHECKLIST.md).
- `../README.md` (2025-11) → superseded by this file.

### Out-of-scope this round

- `../08-test-security-audit.md` (2025-11) — test-suite security audit. Not re-audited; consult original.

---

## Methodology

- **Standard:** OWASP ASVS 4.0.3 — target Level 2.
- **Approach:** One reviewer per module / area, parallel batches. Each reviewer was given full read access to the relevant source under `src-app/server/` and the prior audit (`.sec-audits/`) where applicable.
- **Constraints:** Pure read-only review — no `cargo` / `git` / `sqlx` / `docker` / `npm` commands. No source modifications. No tests executed. No dynamic / runtime exploit testing.
- **Output shape:** Each per-module audit file contains: Executive summary (severity counts + top-3 risks) → Findings (`F-NN` with severity, ASVS, CWE, location, description, vulnerable code, exploitation, impact, recommendation) → ASVS coverage matrix → Positive findings → Out-of-scope / deferred items → comparison vs. prior audits where applicable.
- **Severity scheme:**
  - **Critical** — RCE, authn bypass, full data exfiltration without auth, hardcoded production secret. **Fix within 24h.**
  - **High** — Privilege escalation, SSRF, path traversal with arbitrary R/W, stored credential theft, server-wide DoS. **Fix within 1 week.**
  - **Medium** — Limited info disclosure, missing rate limit, weak crypto, CSRF, resource exhaustion, single-user DoS. **Fix within 1 month.**
  - **Low** — Defense-in-depth gaps, weak error messages, missing security headers, cosmetic data-integrity issues. **Backlog.**
  - **Info** — Hardening notes, dead-code observations, doc-drift, positive findings to preserve. **Not vulnerabilities.**
- **Mapping:** Each finding includes the matching ASVS requirement (e.g., `V8.3.4`) and CWE reference (e.g., `CWE-918`) so external tooling (Snyk, JIRA, GitHub Security Advisories) can ingest the list directly.

---

## Known Limitations

- **`cargo audit` not run** — would require a `cargo` invocation outside the read-only constraint. Theme 8 (mixed `hyper 0.14`/`1.7`, `rustls 0.21`/`0.23`, `rand 0.8`/`0.9`) in the executive summary was identified by reading `Cargo.lock` rather than by tooling.
- **No dynamic / runtime testing** — every finding is grounded in source review. Exploit scenarios are described conceptually; actual exploit construction is left to the remediation team's verification step.
- **Frontend (`src-app/ui/`) not audited** — the audit specifically targets the backend HTTP / DB / business-logic surface. Frontend XSS / CSP / store-leak concerns are out of scope.
- **MCP and code_sandbox modules audited separately** — see "Related Audits" above. Findings in this round that touch the MCP / sandbox boundary (e.g., `02-permissions§F-04` MCP elicitation IDOR, `04-chat§F-07` resource_link SSRF) are flagged for traceability but the root-cause work belongs in the dedicated audits.
- **Database-schema RLS / row-level audit** — touched on lightly under `14-core§F-21` (migration `set_ignore_missing`); a dedicated schema-design audit is a separate workstream.
- **Reverse-proxy / TLS-termination assumptions** — assumed delegated to an upstream proxy (nginx, Cloudflare, etc.). Deployment-topology security is out of scope until a formal deployment guide exists.
- **`postgresql_embedded` upstream supply chain** — pinned to `0.20.0` with bundled binaries; full review of `theseus`'s distribution model is out of scope.

---

## How to Read a Finding

Each finding (`F-NN`) in a per-module audit file is structured as:

```
### F-NN — One-line title (Severity)
- **Severity:** Critical | High | Medium | Low | Info
- **ASVS:** V<chapter>.<sub> (e.g., V8.3.4 — short description of the requirement)
- **CWE:** CWE-NNN (Common Weakness Enumeration reference)
- **Location:** path/to/file.rs:LINE_RANGE (most-actionable location; finding may span multiple sites)
- **Description:** Why this is a problem.
- **Vulnerable code:** ```rust ... ``` snippet from the source.
- **Exploitation:** Concrete attacker scenario (preconditions, request shape, observable effect).
- **Impact:** Confidentiality / integrity / availability impact; blast radius (single user vs. cross-tenant vs. server-wide).
- **Recommendation:** Specific fix, often with code sketch.
- **(Optional) Reference:** External standard, RFC, CVE, prior audit cross-reference.
```

Findings are numbered sequentially within each audit file (`F-01`, `F-02`, …). Severity is **never** implied by the F-number — `F-01` is the first finding, not necessarily the most severe (though most audits do open with their most severe finding).

The [`REMEDIATION-CHECKLIST.md`](./REMEDIATION-CHECKLIST.md) collapses each finding to a single line; the per-module audit files are the canonical source for full context.

---

## Re-audit Cadence Recommendation

Based on the carryover pattern observed (multiple Critical/High findings remain unfixed across two audit cycles, e.g. `01-auth§F-01`, `04-chat§F-01`, `06-llm-provider§F-01`, `14-core§F-01`/`F-04`/`F-13`), the project would benefit from a more aggressive remediation cadence rather than a more frequent re-audit cadence. Recommended schedule:

- **Per-PR delta audit** for security-sensitive modules (`auth`, `permissions`, `user`, `file`, `llm-provider`, `llm-repository`, `core-infrastructure`, `mcp`, `code_sandbox`). Use the existing typed `PermissionCheck` and OpenAPI annotation patterns to make security-relevant changes self-flagging in code review.
- **Full re-audit every 6 months** OR after any major refactor of the audit's hot zones (auth flow, RBAC engine, file/upload pipeline, llm-provider key handling, sandbox boundary). 2026-11 would be the next scheduled full round.
- **Immediate audit after** any change to: JWT issuance / validation, OAuth callback handling, multipart upload routing, secret storage layer, RBAC permission set, or sandbox `--clearenv` / network-namespace policy.
- **Annual external pen-test** to cover the dynamic / runtime testing dimension that this read-only round cannot.

A pragmatic process improvement that would close many of the carryover findings without changing the audit cadence: **define a remediation SLA per severity tier** (Critical: 1 sprint, High: 2 sprints, Medium: 1 quarter), track on the team's project board, and make outstanding Critical/High findings **block release tagging** until remediated.

---

**End of README.**
