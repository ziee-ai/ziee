# Security Remediation Checklist
**Ziee Chat Application**
**Generated:** 2025-11-21

Use this checklist to track security remediation progress. Check off items as they are completed.

---

## 🔴 CRITICAL - Fix Immediately (0-24 hours)

### Day 1 - Emergency Response

- [ ] **CRITICAL-01: Remove API Keys from Responses**
  - [ ] Create `LlmProviderResponse` struct without `api_key` field
  - [ ] Update `list_providers()` handler to use new response type
  - [ ] Update `get_provider()` handler to use new response type
  - [ ] Update `create_provider()` handler to use new response type
  - [ ] Update `update_provider()` handler to use new response type
  - [ ] Test all provider endpoints
  - [ ] Verify API keys no longer in responses
  - **Reference:** `04-llm-modules-audit.md` CRITICAL-01
  - **Files:** `src/modules/llm_provider/handlers.rs`

- [ ] **CRITICAL-02: Disable or Sandbox Pandoc LaTeX**
  - [ ] OPTION A: Switch to weasyprint PDF engine
  - [ ] OPTION B: Add `--no-shell-escape` to pdflatex
  - [ ] OPTION C: Run LaTeX in sandboxed container
  - [ ] Test Office document conversion
  - [ ] Verify no command execution possible
  - **Reference:** `03-file-module-audit.md` CRITICAL-01
  - **Files:** `src/modules/file/services/ocr.rs:170-180`

- [ ] **CRITICAL-03: Fix File Extension Path Traversal**
  - [ ] Create file extension allowlist: `[".pdf", ".txt", ".png", ".jpg", ".jpeg", ".gif", ".doc", ".docx"]`
  - [ ] Validate extensions against allowlist
  - [ ] Reject files with multiple extensions (e.g., `file.php.jpg`)
  - [ ] Add magic bytes validation
  - [ ] Test with malicious filenames
  - **Reference:** `03-file-module-audit.md` CRITICAL-02
  - **Files:** `src/modules/file/handlers/upload.rs:120-125`

- [ ] **CRITICAL-04: Re-enable Request Body Size Limits**
  - [ ] Uncomment body size limit middleware
  - [ ] Set default limit to 10MB
  - [ ] Set file upload limit to 100MB
  - [ ] Add per-route custom limits if needed
  - [ ] Test with large payloads
  - **Reference:** `07-core-infrastructure-audit.md` CRITICAL-01
  - **Files:** `src/core/middleware/mod.rs:45`

### Days 2-3 - Authorization & SSRF Fixes

- [ ] **CRITICAL-05: Fix MCP Session Authorization Bypass**
  - [ ] Add `verify_user_can_access_server()` function
  - [ ] Check user group membership before session creation
  - [ ] Verify server is assigned to user's groups
  - [ ] Add integration tests for unauthorized access
  - [ ] Test cross-user MCP access scenarios
  - **Reference:** `05-mcp-module-audit.md` CRITICAL-01
  - **Files:** `src/modules/mcp/session.rs:45-85`

- [ ] **CRITICAL-06: Add SSRF Protection to MCP**
  - [ ] Create `is_private_ip()` helper function
  - [ ] Block 127.0.0.0/8 (localhost)
  - [ ] Block 10.0.0.0/8 (private network)
  - [ ] Block 172.16.0.0/12 (private network)
  - [ ] Block 192.168.0.0/16 (private network)
  - [ ] Block 169.254.0.0/16 (link-local)
  - [ ] Block IPv6 equivalents (::1, fc00::/7, fe80::/10)
  - [ ] Add URL scheme allowlist (http, https only)
  - [ ] Test with malicious URLs
  - **Reference:** `05-mcp-module-audit.md` CRITICAL-02
  - **Files:** `src/modules/mcp/transports/http.rs:30-45`

- [ ] **CRITICAL-07: Fix OAuth Token URL Exposure**
  - [ ] Use POST request body for token instead of URL
  - [ ] OR use fragment-based redirect (#token=...)
  - [ ] Clear browser history after token exchange
  - [ ] Add test for token exposure
  - **Reference:** `01-auth-user-permissions-audit.md` CRITICAL-01
  - **Files:** `src/modules/auth/oauth/handlers.rs:190-195`

- [ ] **CRITICAL-08: Secure Database Password**
  - [ ] Use environment variable for build DB password
  - [ ] Remove password from `build.rs` print statements
  - [ ] Update build documentation
  - [ ] Test build process with new configuration
  - **Reference:** `07-core-infrastructure-audit.md` CRITICAL-02
  - **Files:** `build.rs:67-70`

---

## 🟠 HIGH - Fix This Week (1-7 days)

### Authentication & Secrets

- [ ] **HIGH-01: Implement Rate Limiting on Auth Endpoints**
  - [ ] Add rate limiting middleware (recommend: `governor` crate)
  - [ ] Limit login attempts: 5 per 15 minutes per IP
  - [ ] Limit registration: 3 per hour per IP
  - [ ] Limit password reset: 3 per hour per email
  - [ ] Add rate limit headers (X-RateLimit-*)
  - [ ] Return 429 Too Many Requests when exceeded
  - [ ] Add tests for rate limiting
  - **Reference:** `01-auth-user-permissions-audit.md` HIGH-01
  - **Files:** `src/modules/auth/handlers/*`

- [ ] **HIGH-02: Strengthen JWT Secret**
  - [ ] Validate JWT secret length >= 32 bytes
  - [ ] Fail fast if weak secret detected
  - [ ] Document secret generation (use `openssl rand -hex 32`)
  - [ ] Add secret rotation mechanism
  - [ ] Update configuration documentation
  - **Reference:** `07-core-infrastructure-audit.md` HIGH-03
  - **Files:** `src/core/config.rs`

- [ ] **HIGH-03: Implement JWT Token Revocation**
  - [ ] Create `revoked_tokens` table (token_jti, revoked_at, expires_at)
  - [ ] Add middleware to check revocation list
  - [ ] Implement logout endpoint that revokes token
  - [ ] Add cleanup job for expired revocations
  - [ ] Add tests for token revocation
  - **Reference:** `01-auth-user-permissions-audit.md` HIGH-03

- [ ] **HIGH-04: Enforce Password Strength**
  - [ ] Implement password validation function
  - [ ] Minimum 12 characters (currently 8)
  - [ ] Require uppercase, lowercase, number, special char
  - [ ] Check against common password list (zxcvbn crate)
  - [ ] Return clear error messages
  - [ ] Add tests for password validation
  - **Reference:** `01-auth-user-permissions-audit.md` HIGH-04
  - **Files:** `src/modules/user/handlers.rs`

### SSRF & Credential Exposure

- [ ] **HIGH-05: Add SSRF Protection to Repository Downloads**
  - [ ] Block private IP ranges (same as MCP)
  - [ ] Implement URL allowlist for Git hosts
  - [ ] Validate URL scheme (http/https only)
  - [ ] Add timeout for HTTP requests (30s max)
  - [ ] Test with malicious repository URLs
  - **Reference:** `04-llm-modules-audit.md` HIGH-03
  - **Files:** `src/modules/llm_repository/handlers.rs`

- [ ] **HIGH-06: Remove Repository Credentials from Responses**
  - [ ] Create `RepositoryResponse` without `auth_config`
  - [ ] Never return passwords, tokens, or API keys
  - [ ] Update all repository endpoints
  - [ ] Add tests to verify credentials not exposed
  - **Reference:** `04-llm-modules-audit.md` HIGH-02
  - **Files:** `src/modules/llm_repository/handlers.rs`

### File Upload Security

- [ ] **HIGH-07: Implement Magic Bytes Validation**
  - [ ] Add `infer` crate for MIME detection
  - [ ] Validate actual file type matches extension
  - [ ] Reject mismatched files (e.g., EXE disguised as PNG)
  - [ ] Add allowlist of safe MIME types
  - [ ] Test with disguised executables
  - **Reference:** `03-file-module-audit.md` HIGH-01
  - **Files:** `src/modules/file/services/validation.rs`

### MCP Security

- [ ] **HIGH-08: Restrict MCP Command Allowlist**
  - [ ] Remove `python` and `node` from allowlist
  - [ ] Only allow specific safe binaries
  - [ ] Validate all command arguments
  - [ ] Add argument escaping/sanitization
  - [ ] Document allowed commands
  - **Reference:** `05-mcp-module-audit.md` HIGH-01
  - **Files:** `src/modules/mcp/transports/stdio.rs`

- [ ] **HIGH-09: Fix MCP Approval Forgery**
  - [ ] Verify conversation ownership in approval handlers
  - [ ] Check user owns conversation before approval
  - [ ] Add defense-in-depth in repository layer
  - [ ] Add tests for cross-user approval attempts
  - **Reference:** `05-mcp-module-audit.md` HIGH-02
  - **Files:** `src/modules/chat/extensions/mcp/approval/handlers.rs`

### Infrastructure

- [ ] **HIGH-10: Configure CORS Properly**
  - [ ] Define allowlist of permitted origins
  - [ ] Remove "allow all origins" default
  - [ ] Configure allowed methods (GET, POST, etc.)
  - [ ] Configure allowed headers
  - [ ] Set credentials policy
  - [ ] Test CORS from unauthorized origin
  - **Reference:** `07-core-infrastructure-audit.md` HIGH-04
  - **Files:** `src/core/middleware/cors.rs`

---

## 🟡 MEDIUM - Fix This Month (1-4 weeks)

### Input Validation & Injection

- [ ] **MEDIUM-01: Prevent User Enumeration**
  - [ ] Return same message for invalid user/password
  - [ ] Add artificial delay to failed login (200-500ms)
  - [ ] Consistent timing for valid/invalid users
  - [ ] Update registration error messages
  - **Reference:** `01-auth-user-permissions-audit.md` MEDIUM-01

- [ ] **MEDIUM-02: Sanitize Error Messages**
  - [ ] Create custom error response type
  - [ ] Never expose database errors
  - [ ] Never expose internal paths
  - [ ] Log full errors server-side only
  - [ ] Return generic messages to client
  - **Reference:** `07-core-infrastructure-audit.md` MEDIUM-02

- [ ] **MEDIUM-03: Implement CSRF Protection**
  - [ ] Add CSRF token generation
  - [ ] Validate tokens on state-changing requests
  - [ ] Use SameSite=Strict on cookies
  - [ ] Document CSRF protection
  - **Reference:** `01-auth-user-permissions-audit.md` MEDIUM-03

- [ ] **MEDIUM-04: Prevent LDAP Injection**
  - [ ] Sanitize LDAP filter input
  - [ ] Escape special characters: `*()\\`
  - [ ] Use parameterized LDAP queries if possible
  - [ ] Add tests for injection attempts
  - **Reference:** `01-auth-user-permissions-audit.md` MEDIUM-06

### File Security

- [ ] **MEDIUM-05: Add Path Canonicalization**
  - [ ] Use `std::fs::canonicalize()` for all file paths
  - [ ] Verify canonical path is within allowed directory
  - [ ] Reject paths with `..` or symlinks
  - [ ] Test with directory traversal attempts
  - **Reference:** `03-file-module-audit.md` MEDIUM-01

- [ ] **MEDIUM-06: Implement Upload Rate Limiting**
  - [ ] Limit uploads per user (e.g., 10 per hour)
  - [ ] Limit total storage per user (e.g., 1GB)
  - [ ] Track upload bandwidth usage
  - [ ] Return 429 when limits exceeded
  - **Reference:** `03-file-module-audit.md` MEDIUM-02

- [ ] **MEDIUM-07: Add ZIP Bomb Protection**
  - [ ] Check compression ratio before extraction
  - [ ] Limit uncompressed size (e.g., 10:1 ratio max)
  - [ ] Limit nested archive depth
  - [ ] Test with ZIP bombs
  - **Reference:** `03-file-module-audit.md` MEDIUM-03

### Chat & MCP

- [ ] **MEDIUM-08: Fix Branch Ownership TOCTOU**
  - [ ] Use database transaction for check+action
  - [ ] Implement optimistic locking
  - [ ] Add version column to conversations
  - [ ] Test concurrent access scenarios
  - **Reference:** `02-chat-module-audit.md` HIGH-01

- [ ] **MEDIUM-09: Add MCP Rate Limiting**
  - [ ] Limit tool calls per conversation (e.g., 100/hour)
  - [ ] Limit total tool execution time (e.g., 5min/hour)
  - [ ] Track resource usage per user
  - [ ] Return error when limits exceeded
  - **Reference:** `05-mcp-module-audit.md` HIGH-04

- [ ] **MEDIUM-10: Implement MCP Tool Argument Validation**
  - [ ] Validate arguments against JSON schema
  - [ ] Reject unexpected properties
  - [ ] Validate data types and ranges
  - [ ] Sanitize string arguments
  - **Reference:** `05-mcp-module-audit.md` HIGH-03

### Infrastructure & Monitoring

- [ ] **MEDIUM-11: Add Security Headers**
  - [ ] X-Frame-Options: DENY
  - [ ] X-Content-Type-Options: nosniff
  - [ ] X-XSS-Protection: 1; mode=block
  - [ ] Strict-Transport-Security (HSTS)
  - [ ] Content-Security-Policy
  - **Reference:** `07-core-infrastructure-audit.md` LOW-03

- [ ] **MEDIUM-12: Implement Audit Logging**
  - [ ] Log all authentication events
  - [ ] Log authorization failures
  - [ ] Log sensitive data access
  - [ ] Log administrative actions
  - [ ] Include user ID, IP, timestamp
  - **Reference:** `05-mcp-module-audit.md` MEDIUM-02

---

## 🔵 LOW - Nice to Have (Backlog)

### Authentication Improvements

- [ ] **LOW-01: Specify JWT Algorithm Explicitly**
  - [ ] Set algorithm to HS256 explicitly
  - [ ] Reject "none" algorithm
  - [ ] Validate algorithm on decode
  - **Reference:** `01-auth-user-permissions-audit.md` LOW-01

- [ ] **LOW-02: Implement Account Lockout**
  - [ ] Lock account after 10 failed login attempts
  - [ ] Lockout duration: 30 minutes
  - [ ] Allow unlock via email
  - [ ] Notify user of lockout
  - **Reference:** `01-auth-user-permissions-audit.md` LOW-02

- [ ] **LOW-03: Add Email Verification**
  - [ ] Send verification email on registration
  - [ ] Generate secure verification token
  - [ ] Verify email before account activation
  - [ ] Resend verification option
  - **Reference:** `01-auth-user-permissions-audit.md` LOW-05

- [ ] **LOW-04: Increase Bcrypt Cost**
  - [ ] Update from cost 12 to 14
  - [ ] Make cost configurable
  - [ ] Test performance impact
  - [ ] Document reasoning
  - **Reference:** `01-auth-user-permissions-audit.md` LOW-07

### Testing & Documentation

- [ ] **LOW-05: Add Security Test Suite**
  - [ ] SQL injection tests
  - [ ] XSS tests
  - [ ] CSRF tests
  - [ ] Path traversal tests
  - [ ] Rate limiting tests
  - [ ] Authorization boundary tests
  - **Reference:** `08-test-security-audit.md` HIGH-02

- [ ] **LOW-06: Create Security Documentation**
  - [ ] Secure coding guidelines
  - [ ] Threat model documentation
  - [ ] Security architecture diagrams
  - [ ] Incident response procedures
  - **Reference:** All reports

### Monitoring & Compliance

- [ ] **LOW-07: Set Up Security Monitoring**
  - [ ] Deploy SIEM solution
  - [ ] Configure alerting rules
  - [ ] Monitor for suspicious patterns
  - [ ] Set up log aggregation
  - **Reference:** Executive Summary

- [ ] **LOW-08: Implement Compliance Logging**
  - [ ] GDPR: Log data access/deletion
  - [ ] SOC 2: Audit trail for all operations
  - [ ] PCI DSS: Track payment operations (if applicable)
  - **Reference:** Executive Summary

---

## 📊 Progress Tracking

### Overall Progress

- Critical: [ ] 0/8 completed (0%)
- High: [ ] 0/16 completed (0%)
- Medium: [ ] 0/24 completed (0%)
- Low: [ ] 0/22 completed (0%)

**Total: [ ] 0/70 completed (0%)**

### Phase Completion

- [ ] Phase 1: Critical Fixes (Week 1)
- [ ] Phase 2: High Priority (Weeks 2-3)
- [ ] Phase 3: Medium Priority (Weeks 4-7)
- [ ] Phase 4: Low Priority (Weeks 8-12)

### Milestone Checkpoints

- [ ] All CRITICAL issues resolved
- [ ] All HIGH issues resolved
- [ ] Penetration test completed
- [ ] Security regression tests passing
- [ ] Production deployment approved

---

## 📝 Notes

Use this section to track blockers, decisions, and progress updates:

```
[Date] - [Your Name]
- Started Phase 1 remediation
- Blocked on: [describe blocker]
- Completed: [list completed items]
- Next: [what's next]
```

---

**Last Updated:** 2025-11-21
**Next Review:** [Add date for next security review]
