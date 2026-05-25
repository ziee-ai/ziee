# Security Audit — Executive Summary (Fresh Round, 2026-05)

**Date:** 2026-05-23
**Auditor:** Claude (general-purpose, ASVS-aligned multi-agent review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target
**Scope:** `src-app/server/` — 14 module/core audits (~58,000 LOC); MCP and code_sandbox covered separately

---

## Closure Status — `security/remediation-2026-05` (2026-05-23/24)

**Headline: 11 of 11 Critical findings closed. ~30 of 49 High closed. ~50 of 85 Medium closed (incl. ones covered by cross-cutting A1/A2/A3/A5).**

The PR `security/remediation-2026-05` covers ~105 of the 145 C/H/M findings via 5 cross-cutting closures (A1 error redaction, A2 outbound URL validator, A3 middleware stack, A4 permission split, A5 pgcrypto at-rest encryption) plus per-module residue commits. See `REMEDIATION-CHECKLIST.md` for the full SHA-annotated list.

**Deferred (separate PRs):**
- `08-llm-local-runtime F-01` (engine binary cosign) — needs GitHub Actions OIDC for the runtime release pipeline
- `08-llm-local-runtime F-04` (engine 127.0.0.1 port auth) — sizable plumbing through ai-providers crate
- `08-llm-local-runtime F-07` (concurrent-engine quota / cgroup) — needs config + global counter
- `05-file F-16` (per-user storage quota) — needs `count_user_bytes` query + config
- `14-core F-15` (EventBus backpressure refactor)
- `14-core F-16` (build-time download signing) — needs Actions OIDC
- `14-core F-07/F-08` (unused/old deps) — dep-hygiene PR
- `01-auth F-12` (registration email-verify) — feature work
- `02-permissions F-05` (permission caching) — perf, not security
- Several smaller Mediums in 01-auth (timing-based enumeration F-06, OAuth state binding F-08, etc.) and 06-llm-provider

**N/A per data model (not actual findings):**
- `07-llm-model F-04` (per-user model ownership) — `llm_models` is admin-curated system-wide
- `07-llm-model F-10` (download hijack) — `download_instances` is admin-driven; `download_instances_write` not in default Users group
- `07-llm-model F-11` (SSE broadcast to all clients) — `download_instances_read` not in default Users group; subscribers are admins
- `06-llm-provider F-04` (cross-tenant llm_provider_files) — already gated via `files.user_id` JOIN (commit 4dd543a)
- `04-chat F-10` (send_message to non-active branch) — by-design: sending activates the targeted branch

---

## TL;DR

The Ziee Chat server has a **structurally sound RBAC and SQL layer** (typed `RequirePermissions<T>` extractors, 100% parameterised `sqlx::query!`, fail-closed extractor design, PKCE + algorithm-pinned JWT) but ships with **system-wide operational hardening gaps** that turn a single low-privilege account into a server compromise. **11 Critical** and **49 High** findings concentrate in five repeatable categories: plaintext secret storage and echo, missing SSRF defences on outbound URLs, no body-size or rate limits anywhere in the router stack, raw `sqlx::Error` text leaked through `AppError::database_error`, and three permission strings (`users::edit`, `groups::edit`, `groups::assign_users`) that each escalate to wildcard `*` from a single grant. Most of the prior-round (2025-01 / 2025-11) Critical and High findings remain **unfixed** at the time of re-audit. **Recommended Phase-1 (24h–1 week) action list: F-01 of `01-auth` (OAuth token-in-URL), F-01/F-02 of `03-user` and `02-permissions` (RBAC escalation triple), F-01 of `05-file` (Pandoc-via-pdflatex RCE), F-01/F-02 of `06-llm-provider` (API-key leak + plaintext), F-01 of `07-llm-model` and `09-llm-repository` (SSRF + HF-token exfiltration), and F-01/F-03/F-05/F-06 of `14-core-infrastructure` (body limit, JWT secret, rate limit, CORS).**

---

## Severity Distribution

| Severity | Count | Description |
|---|---:|---|
| **Critical** | **11** | RCE, authn bypass, full data exfil without auth, hardcoded production secret. **Fix within 24h.** |
| **High** | **49** | Privilege escalation, SSRF, path traversal with arbitrary R/W, stored credential theft. **Fix within 1 week.** |
| **Medium** | **85** | Limited info disclosure, missing rate limits, weak crypto, CSRF, resource exhaustion. **Fix within 1 month.** |
| **Low** | **65** | Defense-in-depth gaps, weak error messages, missing security headers. **Backlog.** |
| **Info** | **46** | Hardening notes; not vulnerabilities. |
| **Total** | **256** | |

---

## Severity by Module

| # | Module | LOC | Critical | High | Medium | Low | Info | Total |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 01 | auth | 2,824 | 1 | 6 | 7 | 4 | 2 | 20 |
| 02 | permissions | 758 | 0 | 4 | 6 | 5 | 4 | 19 |
| 03 | user | 2,026 | 2 | 4 | 6 | 5 | 4 | 21 |
| 04 | chat | 11,012 | 1 | 3 | 7 | 5 | 3 | 19 |
| 05 | file | 3,292 | 1 | 5 | 11 | 6 | 4 | 27 |
| 06 | llm-provider | 2,195 | 2 | 4 | 7 | 5 | 4 | 22 |
| 07 | llm-model | 5,045 | 1 | 5 | 6 | 5 | 3 | 20 |
| 08 | llm-local-runtime | 2,951 | 0 | 4 | 5 | 5 | 2 | 16 |
| 09 | llm-repository | 1,169 | 1 | 5 | 6 | 4 | 1 | 17 |
| 10 | assistant | 1,644 | 0 | 1 | 3 | 4 | 1 | 9 |
| 11 | hub | 2,116 | 0 | 0 | 4 | 4 | 7 | 15 |
| 12 | hardware | 1,529 | 0 | 2 | 3 | 3 | 3 | 11 |
| 13 | misc (app/health/onboarding) | 649 | 0 | 2 | 4 | 5 | 4 | 15 |
| 14 | core-infrastructure | ~3,000 | 2 | 4 | 10 | 5 | 4 | 25 |
| | **Total** | **~40,210** | **11** | **49** | **85** | **65** | **46** | **256** |

---

## Top 10 Critical / High Risks (Prioritised)

These are the findings whose fix order is most material to lowering deployment risk in the next 1–2 weeks. Each entry points at the audit file (`NN-name.md§F-NN`) and the most-actionable file:line.

1. **OAuth callback delivers JWT in URL query string** — `01-auth§F-01`
   `modules/auth/handlers.rs:572-575` — `Redirect::temporary("/?token=…")`. Token in browser history, `Referer`, access logs, address bar. Combined with no revocation (`§F-02`) and no rotation (`§F-03`), a single OAuth-on-shared-machine event is account takeover for 24 h, admin-takeover if the user was admin. Carryover from 2025-11.

2. **Pandoc-with-pdflatex RCE on file upload** — `05-file§F-01`
   `modules/file/utils/pandoc.rs:39-44` — pdflatex invoked without `-no-shell-escape`. Any DOCX / DOC / RTF / ODT / PPTX a user uploads is converted to LaTeX and rendered with the implicit-default shell-escape, reaching `\write18{curl … | sh}`. RCE as the server uid → reads every user's files, steals `JWT_SECRET`, pivots to Postgres via `DATABASE_URL`.

3. **`users::edit` / `groups::edit` / `groups::assign_users` are each a one-permission root escalation** — `03-user§F-01`, `02-permissions§F-01/F-02`
   `modules/user/handlers/user.rs:164-230`, `modules/user/handlers/groups.rs:127-174,269-292`. `UpdateUserRequest.permissions: Vec<String>` is written verbatim with no allow-list and no "cannot grant higher than self" check. Submitting `{"permissions":["*"]}` to `/users/<self>` flips the caller to wildcard. The system "Users" group's permissions can be rewritten the same way (cascade to every user). Assigning oneself to "Administrators" group requires only `groups::assign_users`. Three independent paths to root.

4. **Root admin can be hard-deleted by any `users::delete` holder** — `03-user§F-02`
   `modules/user/handlers/user.rs:339-358`. No `is_admin` guard in `delete_user` (the guard exists in `update_user` and `toggle_user_active`, just not delete). One DELETE removes the only `is_admin=true` row; combined with `user_groups.assigned_by REFERENCES users(id)` lacking ON DELETE, surfaces a 500 with the raw FK error.

5. **System provider `api_key` echoed plaintext to every authenticated user** — `06-llm-provider§F-01`
   `modules/llm_provider/models.rs:28-45` + `handlers/user.rs:23-67`. `LlmProvider.api_key: Option<String>` with `skip_serializing_if = is_none` (not `skip_serializing`). Every regular user with `user_llm_providers::read` (default seed) reads every system provider's plaintext OpenAI/Anthropic key via `GET /user-llm-providers`. Integration tests *enforce* the leak. Worth thousands of dollars per month per stolen system key.

6. **All provider/repository API keys stored as plaintext `TEXT` in DB** — `06-llm-provider§F-02`, `09-llm-repository§F-02`
   `migrations/00000000000003,028,002`. `api_key`, `password`, `token` columns are `TEXT NOT NULL` with no encryption, no `pgcrypto`, no envelope, no KMS. Backup, read replica, analytics, container escape, SQLi anywhere in the codebase = total credential blast.

7. **SSRF + HF-token exfiltration via `LlmRepository.url`** — `07-llm-model§F-01`, `09-llm-repository§F-01`
   `modules/llm_repository/utils.rs:14-23` — `validate_url` is `reqwest::Url::parse(url).is_ok()`. `file://`, `ssh://`, `git://`, `http://169.254.169.254/`, RFC-1918 all accepted. `clone_repository` passes URL to `git2::RepoBuilder::clone` with the stored auth token attached via `Cred::userpass_plaintext(_, token)` ignoring the actual host. The LFS batch-API's server-returned `action.download.href` is fetched with `url.set_password(token)` injecting the HF token into an attacker-supplied URL. The `auth_test_api_endpoint` field bypasses even the trivial validator since it's never validated.

8. **LFS path-traversal via attacker-controlled `oid`** — `07-llm-model§F-02`
   `utils/git/lfs/service.rs:300-313` + `metadata.rs:48-65`. The OID parser takes the last whitespace token of the `oid sha256:` line with no hex-character check. `tmp_path = PathBuf::from("./").join(format!("{oid}.lfstmp"))` resolves to `./../../../etc/cron.d/payload.lfstmp`; the code then calls `fs::remove_file` on it before the temp-file creation fails. Arbitrary file deletion as server uid from a hostile remote repo.

9. **Global `DefaultBodyLimit::disable()` on every route** — `14-core-infrastructure§F-01`
   `src/main.rs:172`, `src/lib.rs:197`. Comment says "for model uploads (very large)" — applied globally instead of per-route. `curl -X POST -d "$(head -c 50G /dev/urandom)" /api/auth/login` OOM-kills the server with **no authentication required**. Also enables `05-file§F-02`, `07-llm-model§F-03` (multipart fields buffered into `Vec<u8>` before size check).

10. **JWT secret accepts the public example value with zero entropy / length validation** — `01-auth§F-10`, `14-core-infrastructure§F-03`
    `modules/auth/jwt.rs:39-50`, `config/dev.yaml:81`. `JwtService::new` accepts any `config.jwt.secret` string. `dev.yaml` ships `"dev-secret-change-in-production-min-32-chars-long"` (49 chars but identifiable); `prod.example.yaml` ships `"REPLACE_ME_WITH_A_LONG_RANDOM_SECRET_AT_LEAST_32_CHARS"`. An operator who forgets the override boots with a fully public HMAC key — attacker forges `is_admin:true` token offline. No boot-time guard.

**Honourable mentions** that didn't fit the top 10 but are imminent attacker primitives:

- `01-auth§F-04` LDAP filter injection (no RFC 4515 escape).
- `02-permissions§F-03` Download-token JWT shares the access-token signing key with no `iss`/`aud`.
- `04-chat§F-01` Pending-approvals endpoint missing ownership check — leaks tool inputs (carryover, ~16 months unfixed).
- `05-file§F-03` Path traversal via uploaded-filename extension into `originals/<uid>/{uuid}.<ext>`.
- `05-file§F-06` `download_with_token` does not recheck `FilesDownload` permission or user-active state.
- `07-llm-model§F-04` No `created_by` on `llm_models` / `download_instances` — every user sees every other user's downloads, can cancel them, can hijack their `request_data`.
- `08-llm-local-runtime§F-01` Engine binaries downloaded from GitHub with no SHA-256 / cosign verification (sigstore is already wired up for sandbox-rootfs in this repo — pattern not reused).
- `08-llm-local-runtime§F-03` Spawned engine inherits the server's full environment (`DATABASE_URL`, `JWT_SECRET`, `*_API_KEY`); no `env_clear()`, no `PR_SET_PDEATHSIG`.
- `14-core-infrastructure§F-06` Zero rate limiting anywhere in the server (no `tower-governor` in `Cargo.toml`).

---

## Cross-Cutting Themes

The single most valuable observation surfacing across 14 separate audits is that **the same five-or-six root causes account for 60–70% of all findings**. Fixing them centrally closes dozens of findings at once.

### Theme 1 — `AppError::database_error` propagates raw `sqlx::Error` text to clients

**Spans:** 7 of 14 modules.
**Root cause:** `common/type.rs:109-115`:
```rust
pub fn database_error(err: impl std::error::Error) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, "SYSTEM_DATABASE_ERROR",
              format!("Database error: {}", err))
}
```

Constraint names, column names, table names, the offending value, and (sometimes) SQL fragments land in the JSON response body. Schema fingerprinting + free SQLi-like oracle.

**Findings:** `01-auth§F-14`, `03-user§F-08`, `07-llm-model§F-05`, `11-hub§F-03`, `13-misc§F-03/F-15`, `14-core-infrastructure§F-13`. The `06-llm-provider§F-06` (`eprintln!` of `sqlx::Error`) and `09-llm-repository§F-13` (same) are the log-side of the same defect.

**One-place fix:** rewrite `database_error` to log via `tracing::error!` and return `{ error_code: "SYSTEM_DATABASE_ERROR", message: "Database operation failed", correlation_id: <uuid> }`. Closes 8+ findings.

### Theme 2 — No rate limiting, no body limit, no request timeout anywhere

**Spans:** every module that accepts a request.
**Root cause:** No `tower-governor` / `tower::limit::RateLimitLayer` / `TimeoutLayer` / per-route `DefaultBodyLimit` in any router file. The one body-limit decision was to **globally disable** it (`14-core§F-01`).

**Findings:** `01-auth§F-05`, `02-permissions§F-18`, `03-user§F-12`, `04-chat§F-04`, `05-file§F-02/F-17`, `06-llm-provider§F-13`, `07-llm-model§F-03`, `08-llm-local-runtime§F-07`, `09-llm-repository§F-05/F-11`, `10-assistant§F-03`, `11-hub§F-06/F-07`, `12-hardware§F-01/F-03`, `13-misc§F-01/F-10`, `14-core-infrastructure§F-01/F-05/F-06`. **Eighteen findings, one absent middleware stack.**

**One-place fix:** add `tower-governor` + `TimeoutLayer` + per-route `DefaultBodyLimit::max(...)` at `core/app_builder.rs`. Restrict global body limit to 25 MB; whitelist `/api/llm-models/upload` and `/api/files/upload` to higher caps. Closes 18+ findings.

### Theme 3 — Plaintext secret storage and serialization

**Spans:** llm-provider, llm-repository, llm-provider-files.
**Root cause:** Secrets are `TEXT` columns; structs are `Serialize` with `skip_serializing_if = "Option::is_none"` (which only suppresses `None`, NOT a populated value).

**Findings:** `06-llm-provider§F-01/F-02/F-05` (API key + proxy password in responses, plaintext at rest), `09-llm-repository§F-02/F-04` (HF/GitHub credentials in responses + URL userinfo logged), `02-permissions§F-03` (download-token JWT shares access-token secret).

**One-place fix:** introduce a `SecretView<String>` newtype (via `secrecy` crate) with `Drop` zeroising, no `Serialize` impl, and `Display`/`Debug` that prints `"<redacted>"`. Make every "secret-bearing" struct have separate write-DTOs (accept) vs read-DTOs (don't echo). Add `pgcrypto`-backed encryption at the repository layer. Closes 6+ findings.

### Theme 4 — SSRF / URL validation is `reqwest::Url::parse(s).is_ok()`

**Spans:** llm-provider, llm-model, llm-repository, auth (OIDC UserInfo), chat (MCP resource_link), hub (refresh).
**Root cause:** No centralised URL allowlist helper. Every module that takes a URL does its own check, all of them reduce to "is this a URL?".

**Findings:** `01-auth§F-18` (OAuth UserInfo SSRF), `04-chat§F-07` (resource_link SSRF), `06-llm-provider§F-03/F-05` (provider base_url SSRF + TLS toggle that does nothing), `07-llm-model§F-01/F-06` (repo URL SSRF + libgit2 credential cross-host leak), `09-llm-repository§F-01/F-03/F-08/F-12/F-15` (validate_url accepts everything; auth_test_api_endpoint unvalidated; credential callback ignores host on redirect; no DNS pinning), `11-hub§F-01/F-04` (placeholder GitHub URL + attacker-controlled version path component).

**One-place fix:** ship `utils::url_safety::validate_outbound_url(url, allowlist) -> Result<Url>` with:
- `https` only (except dev-mode `http://localhost`)
- Rejects IP-literal hosts that resolve to loopback / `10.0.0.0/8` / `172.16.0.0/12` / `192.168.0.0/16` / `169.254.0.0/16` / `100.64.0.0/10` / `fc00::/7` / `fe80::/10`
- DNS-pinned via custom `Resolve` impl (defeats rebinding)
- Optional per-caller host allowlist (HF, GitHub.com)
- Used by every outbound `reqwest::Client` AND wraps libgit2's credential callback with same-host check.

Closes 15+ findings.

### Theme 5 — Permission grants that escalate (the RBAC "footgun ladder")

**Spans:** user, permissions, assistant, hub, file (download token), llm-provider (built-in).
**Root cause:** The handler-side validation of `permissions: Vec<String>` / `assign_user_to_group` / "is this a system row" is incomplete. RBAC permissions are correctly checked at the route layer; the **business rules above** are missing.

**Findings:** `02-permissions§F-01/F-02/F-08` and `03-user§F-01/F-02/F-04/F-07` (the escalation triple plus the create variant plus the delete-root-admin gap), `02-permissions§F-09` (dead-code single-colon `:` permission check that silently grants nothing but exists as a future footgun), `02-permissions§F-12` (admin-account enumeration via 400 vs 200), `02-permissions§F-13` (typo'd permission strings accepted), `06-llm-provider§F-15` (`built_in: true` rows editable by `llm_providers::edit`), `09-llm-repository§F-16` (built-in HF/GitHub URLs editable post-bootstrap).

**One-place fix:** split mutating permissions into `<resource>::set_permissions` (defaults to root-admin only) vs the existing `<resource>::edit`. Refuse any permission grant the caller doesn't themselves hold. Add `RequireAdmin`-or-`is_system_owner` guards on system-flagged rows. Audit-log every successful permission change. Closes 8+ findings.

### Theme 6 — Ownership checks done in handlers, not in repository SQL

**Spans:** chat, llm-model, llm-provider-files, assistant, mcp-elicitation (out of scope but called out by `02-permissions§F-04`).
**Root cause:** Most "current" handlers do `verify_message_ownership(id, user_id)` before the actual mutation, but the **repository** functions accept `user_id` as advisory or ignore it. A future contributor wiring a new code path into the same repo function silently bypasses ownership.

**Findings:** `04-chat§F-01/F-09` (pending-approvals + repo trusts caller), `04-chat§F-02`/`10-assistant§F-01` (assistant `get(id)` has no user filter, chat extension exploits it), `06-llm-provider§F-04` (llm_provider_files has no `user_id` column at all), `07-llm-model§F-04/F-10/F-11` (no `created_by` on models or downloads — every user sees and can cancel all downloads).

**One-place fix:** require every state-mutating and ownership-relevant repository function to take `user_id` and fold it into `WHERE ... AND owner_user_id = $N` (or `WHERE EXISTS (SELECT 1 FROM conversations c WHERE c.id = ? AND c.user_id = ?)`). Add a CI lint or PR-template checklist for new repo functions. Closes 7+ findings.

### Theme 7 — Carryover findings from 2025-01 / 2025-11 audits not fixed

**Spans:** auth, chat, file, llm-provider, llm-repository, hub, core-infrastructure.
**Root cause:** Prior audits identified the same root causes. Two audit cycles later they remain.

**Carryover Criticals/Highs:**
- `01-auth§F-01` (OAuth token-in-URL) — carryover from 2025-11
- `01-auth§F-05/F-10` (no rate limit, weak JWT secret) — carryover
- `04-chat§F-01` (pending-approvals leak) — carryover from 2025-01 (~16 months)
- `06-llm-provider§F-01` (API keys in responses) — carryover from 2025-11
- `09-llm-repository§F-02` (HF token in responses) — carryover
- `14-core-infrastructure§F-01/F-04/F-13` (body limit, CORS, DB error) — all carryover from 2025-11

The chat-module CRITICAL-01 finding (`get_pending_approvals_for_branch` missing ownership check) was first flagged on 2025-01-21 and is unchanged in code 16 months later. The leading underscore on the discarded `_auth` parameter makes this a documented omission, not an oversight.

**Recommendation:** Treat the Critical and High Phase-1 list below as a release-blocker for the next tagged version. Carryover beyond two audit cycles indicates a process gap, not a knowledge gap.

---

## ASVS Coverage Summary

Aggregated `pass / partial / fail` votes across the 14 module audits (one chapter can be touched by multiple audits; counts below are the **number of audits that flagged a finding under that chapter**, so they trace remediation priority).

| ASVS Chapter | Pass-leaning | Partial | Fail | Notable findings |
|---|---:|---:|---:|---|
| **V1 — Architecture** | 11 | 2 | 1 | Mostly compile-time-checked typed extractors. |
| **V2 — Authentication** | 3 | 5 | 6 | F-05/F-10/F-12 in auth; F-01 in misc; CIs around password policy, breached-pw check, MFA. |
| **V3 — Session Mgmt** | 4 | 4 | 6 | F-01/F-02/F-03/F-08 in auth (token-in-URL, no revocation, no rotation, state not bound). |
| **V4 — Access Control** | 8 | 3 | 3 | RBAC extractor solid; gates **above** extractor partial (F-01 in 03-user, F-01-F-04 in 02-permissions). |
| **V5 — Validation / Injection** | 2 | 5 | 7 | LDAP injection (auth-F-04), path traversal (file-F-03, llm-model-F-02), no length caps anywhere, no NFC normalisation. |
| **V6 — Stored Cryptography** | 6 | 4 | 4 | bcrypt fine; plaintext secret storage (06/09) is the headline fail. |
| **V7 — Error Handling & Logging** | 1 | 4 | 9 | `AppError::database_error` leaks everywhere; `println!`/`eprintln!` used in modules; no audit log on RBAC changes. |
| **V8 — Data Protection** | 5 | 4 | 5 | `LlmProvider.api_key` in responses, PII in `users::read`, kernel-version disclosure. |
| **V9 — Communication / TLS** | 6 | 3 | 5 | No `https_only`, no redirect cap on outbound clients; `eventsource-client` drags rustls 0.21. |
| **V10 — Malicious Code / Subprocess** | 3 | 5 | 6 | Pandoc-pdflatex RCE; no integrity verification of downloaded binaries (Pandoc, PDFium, llamacpp, mistralrs). |
| **V11 — Business Logic** | 5 | 4 | 5 | No rate limit, no quota, branch / template / onboarding cardinality unbounded. |
| **V12 — Files & Resources** | 2 | 4 | 8 | Path traversal (file-F-03, llm-model-F-02, llm-repo-F-06), no body limit (everywhere), no decompression-bomb protection, no symlink resolution. |
| **V13 — API** | 6 | 3 | 5 | Pagination bounds missing in 7 modules; OpenAPI doesn't declare `securitySchemes`. |
| **V14 — Configuration** | 5 | 4 | 5 | Permissive CORS default; weak JWT example; no security headers; binary fetcher uses `releases/latest` for PDFium. |

**ASVS L2 verdict for the server as a whole: FAIL.** No single module currently passes L2 cleanly. The closest passes are `02-permissions` (extractor design is excellent; gaps are in business rules above) and `12-hardware` (small, well-bounded surface; gaps are operational rather than authn/authz).

---

## What's NOT in this Round

- **MCP module** — covered separately by [`../05-mcp-module-audit.md`](../05-mcp-module-audit.md) (2025-01) and [`../mcp-phase3-i2-get-sse-audit-2026-05-22.md`](../mcp-phase3-i2-get-sse-audit-2026-05-22.md). MCP-server-related findings that touch the chat module (resource_link SSRF, elicitation IDOR) are surfaced under `02-permissions§F-04` and `04-chat§F-07`.
- **code_sandbox module** — covered separately by [`../wsl2-sandbox-prior-art-2026-05-22.md`](../wsl2-sandbox-prior-art-2026-05-22.md) and [`../wsl2-source-deep-read-2026-05-22.md`](../wsl2-source-deep-read-2026-05-22.md).
- **Frontend (`src-app/ui/`)** — out of scope this round.
- **Dependency CVE scan** — `cargo audit` not run per audit constraints. Theme 8 (mixed `hyper 0.14`/`1.7`, `rustls 0.21`/`0.23`) was identified by reading `Cargo.lock` rather than running scanners.
- **Dynamic / runtime exploit testing** — pure code review.
- **Database schema RLS / row-level audit** — touched on lightly; not deep-audited.
- **Reverse-proxy / TLS termination assumptions** — assumed delegated; out of scope.

---

## Recommended Remediation Phases

### Phase 1 — 24h to 1 week (release-blocker)

All Critical + the "top-tier" High findings. These are the exploit chains that hand an attacker root or wallet damage.

| # | Finding | Severity | Owner / Effort |
|---|---|---|---|
| 1 | [`01-auth§F-01`](./01-auth.md) — OAuth token-in-URL | Critical | auth — 0.5d |
| 2 | [`03-user§F-01`](./03-user.md) + [`02-permissions§F-01/F-02`](./02-permissions.md) — RBAC escalation triple | Critical | user/permissions — 1-2d |
| 3 | [`03-user§F-02`](./03-user.md) — root admin deletable | Critical | user — 1h |
| 4 | [`05-file§F-01`](./05-file.md) — Pandoc-pdflatex RCE | Critical | file — 1h (immediate `-no-shell-escape`), 1w (tectonic switch) |
| 5 | [`06-llm-provider§F-01`](./06-llm-provider.md) — API key in responses | Critical | llm-provider — 1d |
| 6 | [`06-llm-provider§F-02`](./06-llm-provider.md) — plaintext at rest | Critical | llm-provider — 1-2w (pgcrypto first, KMS later) |
| 7 | [`07-llm-model§F-01`](./07-llm-model.md) + [`09-llm-repository§F-01`](./09-llm-repository.md) — SSRF + HF-token exfil | Critical | llm-model/llm-repo — 2-3d |
| 8 | [`07-llm-model§F-02`](./07-llm-model.md) — LFS path traversal via OID | High | llm-model — 1h (regex validate `^[0-9a-fA-F]{64}$`) |
| 9 | [`14-core§F-01`](./14-core-infrastructure.md) — global body limit | Critical | core — 1h |
| 10 | [`14-core§F-03`](./14-core-infrastructure.md) — JWT secret validation | High | core/auth — 2h |
| 11 | [`14-core§F-04`](./14-core-infrastructure.md) — CORS default | High | core — 2h |
| 12 | [`14-core§F-05`](./14-core-infrastructure.md) — global request timeout | High | core — 1h |
| 13 | [`14-core§F-06`](./14-core-infrastructure.md) — add rate limiting | High | core — 1d |
| 14 | [`01-auth§F-04`](./01-auth.md) — LDAP filter injection | High | auth — 2h (add RFC 4515 escape) |
| 15 | [`04-chat§F-01`](./04-chat.md) — pending-approvals leak (16 months unfixed) | Critical | chat — 1h |

**Estimate:** 5–7 engineer-days. The single biggest blocker for Phase 1 is the at-rest-encryption story (item 6) — for a near-term release, `pgcrypto`-keyed-by-env-var is sufficient.

### Phase 2 — 1 week to 1 month

Remaining High + structural Mediums.

- [`01-auth§F-02/F-03/F-07/F-12/F-18`](./01-auth.md) — token revocation, refresh rotation, OAuth `redirect_uri` allowlist, email verification, SSRF on UserInfo.
- [`03-user§F-03/F-04/F-05/F-06`](./03-user.md) — email-rewrite verification, password policy, pagination bounds, audit log.
- [`04-chat§F-02/F-03/F-04`](./04-chat.md) — cross-user assistant `get_for_user`, message tree delete, streaming endpoint rate-limit + body-cap.
- [`05-file§F-02/F-03/F-04/F-05/F-06`](./05-file.md) — body limits, path canonicalisation, magic-byte sniffing, decompression-bomb protection, `download_with_token` re-check permissions.
- [`06-llm-provider§F-03/F-04/F-05/F-06`](./06-llm-provider.md) — SSRF on base_url, cross-tenant llm_provider_files, TLS toggle that does nothing, replace `eprintln!`.
- [`07-llm-model§F-03/F-04/F-05/F-06`](./07-llm-model.md) — body limits, per-user ownership, DB-error opacity, libgit2 credential callback host-check.
- [`08-llm-local-runtime§F-01/F-02/F-03/F-04`](./08-llm-local-runtime.md) — binary integrity, argv flag injection, `env_clear()`, per-instance `--api-key`.
- [`09-llm-repository§F-02/F-03/F-04/F-05/F-06`](./09-llm-repository.md) — credentials in responses, scheme allowlist, URL credential stripping, clone-size caps, LFS OID + submodule lockdown.
- [`12-hardware§F-01/F-02`](./12-hardware.md) — SSE client cap, kernel/CPU/driver redaction.
- [`13-misc§F-01/F-10`](./13-misc-small-modules.md) — setup-admin password policy + rate limit; onboarding array cap.
- The cross-cutting `AppError::database_error` redaction (Theme 1) — closes 8+ findings at once.
- The cross-cutting `tower-governor` + `TimeoutLayer` + per-route body limits (Theme 2) — closes 18+ findings at once.

**Estimate:** 3–4 engineer-weeks.

### Phase 3 — 1 to 3 months

Remaining Mediums + cross-cutting refactors:

- Centralised `SecretView<String>` and KMS-backed envelope encryption (Theme 3).
- Centralised `validate_outbound_url` helper + custom DNS resolver (Theme 4).
- Move every ownership check into repository-layer SQL via `get_for_user(id, user_id)` pattern (Theme 6).
- Permission-grant `<resource>::set_permissions` split (Theme 5).
- Migrate off `eventsource-client` to drop the legacy `hyper 0.14`/`rustls 0.21` chain (`14-core§F-08`).
- Decouple `--generate-openapi` from runtime DB init (`14-core§F-14`).
- Audit-log table + structured logging via `tracing` for every state-changing mutation (closes the audit-trail Info findings).
- Migrate from `bcrypt` cost-12 to `argon2id` (closes `01-auth§F-11`).
- Sigstore/cosign verification on all build-helper binary fetches (`14-core§F-16`).

### Phase 4 — backlog

All Low + Info findings. Total ~111 entries. Tackle opportunistically alongside related feature work.

---

## Comparison vs. Prior Audits (2025-01 / 2025-11)

Cross-referenced against the existing `.sec-audits/` content (parent directory):

| Prior round | Findings carried over (unfixed) | Findings closed | New since prior round |
|---|---:|---:|---:|
| 2025-01 (`01`–`08-*-audit.md`) | ~25 | ~10 | n/a (this was first audit) |
| 2025-11 (`00-EXECUTIVE-SUMMARY.md`) | ~20 | ~5 | this audit identifies ~140 new |

**Biggest carryovers (still open):**

- `01-auth-user-permissions-audit.md` (2025-01) CRITICAL-01 token-in-URL → still present as `01-auth§F-01`.
- `02-chat-module-audit.md` (2025-01) CRITICAL-01 pending-approvals leak → still present as `04-chat§F-01`.
- `03-file-module-audit.md` (2025-11) CRITICAL Pandoc-pdflatex → still present as `05-file§F-01`.
- `04-llm-modules-audit.md` (2025-11) CRIT-1 API key in responses → `06-llm-provider§F-01`.
- `07-core-infrastructure-audit.md` (2025-11) §1 (body limit), §3 (JWT secret), §4 (CORS), §7 (DB error) → all unchanged.

**Closed since 2025-11:** notably, the `is_template` immutability hardening in the assistant module (`10-assistant§P-03`), `hub::models::*` removal from default Users group (migration 37), the `user_llm_provider_api_keys` per-user key table (migration 28 — partial mitigation of `06-llm-provider§F-01`).

**Net assessment:** structural design has stayed sound through the year, but **operational hardening has not landed**. The list of changes between 2025-11 and 2026-05 is short; the list of unfixed findings is long.

---

## Positive Findings — System-wide Strengths

Things done well across the audited 14 modules. Preserve these through any remediation.

1. **Compile-time-checked SQL via `sqlx::query!` / `query_as!` macros.** Every audited module uses parameterised, schema-verified queries. **No SQL injection surface anywhere.** This is the single biggest structural-security win in the codebase.

2. **Typed `RequirePermissions<P: PermissionList>` extractor.** Permission strings are compile-time symbols. Misspelling a permission is a compile error, not a runtime auth bypass. AND-semantics on tuples. Fail-closed by Axum's `Result<Self, Err>` contract. (`02-permissions` positive findings P1–P10.)

3. **All 14 modules consistently apply `RequirePermissions<…>` on every state-changing route.** Of ~110 surveyed routes, only 5 are intentionally public (`/health`, `/setup/status`, `/setup/admin`, `/auth/register`, `/auth/login`); all are documented (`02-permissions` route-gating audit).

4. **JWT algorithm pinned to HS256, issuer and audience validated, refresh-token audience distinct.** Prevents `alg=none` and HS/RS confusion. (`01-auth` positive findings.)

5. **PKCE used for OAuth2 and OIDC flows.** State generated server-side via `CsrfToken::new_random()` and stored in `oauth_sessions` with TTL.

6. **`#[serde(skip_serializing)]` on `User.password_hash`** + `#[schemars(skip)]` — the bcrypt hash never appears in any handler response or OpenAPI schema.

7. **Conversation / message / file ownership uses 404 (not 403) on miss** — prevents UUID-existence enumeration.

8. **Cascade DELETE configured at the DB level** for `conversations → branches → branch_messages → messages → message_contents → tool_use_approvals`. No dangling-row classes after a delete.

9. **`#[serde(deny_unknown_fields)]` on `CreateLlmProviderRequest`, `CreateLlmRepositoryRequest`, `CreateLlmModelRequest`, `CreateAssistantRequest`** — blocks attacker-injected JSON fields. (Inconsistently applied on update DTOs — `06-llm-provider§F-08`.)

10. **Single-root-admin invariant enforced at the DB level** via `CREATE UNIQUE INDEX unique_root_admin ON users (is_admin) WHERE is_admin = true`. The race window during initial setup (`13-misc§F-01`) is closed at the DB layer.

11. **`code_sandbox` ships keyless cosign verification of rootfs squashfs via the `sigstore` Rust crate.** The pattern exists in this repo. It is not yet reused for the llm_local_runtime binary fetcher (`08-llm-local-runtime§F-01`) or the build_helper binary fetcher (`14-core§F-16`) — but the team knows how to do it correctly.

12. **Engine processes (when present) bind to `127.0.0.1` only** (`08-llm-local-runtime` positive #3) — no LAN-side exposure.

13. **`module_api` registration via `linkme::distributed_slice` with deterministic ordering** — sorted by `order` field at link time, no runtime module-discovery race. (`14-core` positive #3.)

14. **Compile-time-checked permission strings** (`PermissionCheck` trait) propagate into OpenAPI annotations automatically via `with_permission<P>` — the docs cannot drift from the code.

15. **`postgresql_embedded` is configured with `--rustls`**, not `openssl`. Modern crypto path for the embedded DB. (`14-core` positive #4.)

16. **`code_sandbox_seccomp` is the default-on Linux feature** with operator-friendly documentation. Static-linked libseccomp at build time keeps the runtime install footprint zero. (`14-core` positive #5.)

17. **No `unsafe` blocks anywhere in the audited core paths.** (`14-core` positive #9.)

18. **`Cargo.lock` is checked in for the binary** (correct decision for reproducible builds).

---

## Closing Note

The codebase is **architecturally well-designed**: typed permissions, compile-time-verified SQL, OAuth done with the right libraries and the right ceremony, modular boundaries. The 256 findings cluster around a small number of **central operational gaps** rather than a sea of independent bugs. **Fixing the seven cross-cutting themes above closes ~60–70% of all findings.** The remaining ~30% is per-module business-rule hardening that, while substantial in count, is mechanical work.

The single most impactful change a release manager can make this week:

- **Block the next tagged release until Phase-1 (15 items) is closed.** Five engineer-days of focused work.

The single most impactful change for the next quarter:

- **Land the cross-cutting themes (especially 1, 2, 3, 4) as first-class infrastructure work** rather than letting them be absorbed piecemeal into each module's remediation. Two engineer-weeks for the infrastructure team closes more findings than three engineer-months of per-module patching.

**Carryover beyond two audit cycles indicates a process gap.** Recommendation: define a remediation SLA per severity tier (Critical: 1 sprint, High: 2 sprints, Medium: 1 quarter), tracked in the team's project board, and make security findings unblock-on-merge.

---

**End of executive summary.**
