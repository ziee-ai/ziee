# MCP Module Security Audit

**Date:** 2025-01-21
**Auditor:** Claude (Sonnet 4.5)
**Scope:** Model Context Protocol (MCP) module in `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/`

---

## Executive Summary

The MCP module implements a critical security boundary, allowing external MCP servers to execute arbitrary commands and access resources on behalf of users. This audit identified **12 security vulnerabilities** across command execution, authorization, input validation, and resource management.

**Critical Findings:** 2
**High Severity:** 5
**Medium Severity:** 3
**Low Severity:** 2

### Immediate Actions Required

1. **CRITICAL:** Fix session manager authorization bypass (Finding #1)
2. **CRITICAL:** Implement SSRF protection for HTTP/SSE transports (Finding #4)
3. **HIGH:** Add rate limiting for tool calls (Finding #9)
4. **HIGH:** Validate tool arguments against JSON schema (Finding #6)
5. **HIGH:** Implement approval forgery protection (Finding #5)

---

## Findings

### 1. Session Manager Authorization Bypass

**Severity:** CRITICAL
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/client/manager.rs`
**Lines:** 25-56

**Vulnerability:**

The `McpSessionManager::get_or_create()` method only checks if a server exists and is enabled, but does NOT verify that the requesting user has access to the server. This allows any authenticated user to access any system server, bypassing group-based access controls.

```rust
pub async fn get_or_create(
    &self,
    server_id: Uuid,
) -> Result<Arc<RwLock<McpSession>>, AppError> {
    // Check if session exists
    {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(&server_id) {
            return Ok(session.clone());  // ❌ No access check!
        }
    }

    // Load server config from database
    let repo = McpRepository::new(self.pool.clone());
    let server = repo.get_system_server(server_id).await?  // ❌ Only checks if system server
        .ok_or_else(|| AppError::not_found("Server not found"))?;

    // Check if server is enabled
    if !server.enabled {
        return Err(AppError::bad_request("server_disabled", "Server is disabled"));
    }

    // ❌ No authorization check - any user can access any enabled system server!
    let session = McpSession::new(server).await?;
    // ...
}
```

**Impact:**

- User A in Group X can access MCP servers assigned to Group Y
- Unauthorized tool execution on servers the user should not have access to
- Complete bypass of group-based access control

**Exploitation:**

```bash
# User A discovers server_id of a restricted MCP server
# User A calls /mcp/servers/{restricted_server_id}/tools
# Session manager creates session without checking if User A has access
# User A executes tools on restricted server
```

**Recommended Fix:**

```rust
pub async fn get_or_create(
    &self,
    server_id: Uuid,
    user_id: Uuid,  // Add user_id parameter
) -> Result<Arc<RwLock<McpSession>>, AppError> {
    // Check if session exists
    {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(&server_id) {
            // Verify user still has access before returning cached session
            let repo = McpRepository::new(self.pool.clone());
            if !repo.can_user_access_server(user_id, server_id).await? {
                return Err(AppError::forbidden(
                    "USER_NO_ACCESS",
                    "You do not have access to this server"
                ));
            }
            return Ok(session.clone());
        }
    }

    // Load server and verify access
    let repo = McpRepository::new(self.pool.clone());

    // Check user access BEFORE creating session
    if !repo.can_user_access_server(user_id, server_id).await? {
        return Err(AppError::forbidden(
            "USER_NO_ACCESS",
            "You do not have access to this server"
        ));
    }

    let server = repo.get_system_server(server_id).await?
        .ok_or_else(|| AppError::not_found("Server not found"))?;

    // ... rest of the method
}
```

**Update all callers:**
- `handlers/runtime.rs:71` - Pass `auth.user.id` to `get_or_create()`
- `chat/extensions/mcp/mcp.rs:108, 349, 673` - Pass `context.user_id` to `get_or_create()`

---

### 2. Command Allowlist Too Permissive

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/client/stdio.rs`
**Lines:** 12-13, 54-60

**Vulnerability:**

The command allowlist includes interpreters (`python`, `python3`, `node`, `deno`) that can execute arbitrary code if combined with malicious arguments.

```rust
// Security: Command allowlist (Phase 1)
const ALLOWED_COMMANDS: &[&str] = &["npx", "uvx", "python", "python3", "node", "deno"];

fn create_command(&self) -> Result<Command, AppError> {
    let cmd = self.server_config.command.as_ref()
        .ok_or_else(|| AppError::bad_request("MISSING_COMMAND", "Missing command"))?;

    // Security: Validate command against allowlist
    if !ALLOWED_COMMANDS.contains(&cmd.as_str()) {
        return Err(AppError::bad_request(
            "INVALID_COMMAND",
            &format!("Command '{}' is not allowed", cmd)
        ));
    }

    // ❌ But args are not validated!
    if let Some(arr) = self.server_config.args.as_array() {
        for arg in arr {
            if let Some(arg_str) = arg.as_str() {
                command.arg(arg_str);  // ❌ Arbitrary args accepted
            }
        }
    }
}
```

**Impact:**

Admin creates system MCP server:
```json
{
  "name": "malicious",
  "transport_type": "stdio",
  "command": "python3",
  "args": ["-c", "import os; os.system('rm -rf /')"]
}
```

When any user in assigned groups connects, arbitrary code executes on the server.

**Recommended Fix:**

```rust
// 1. Remove raw interpreters from allowlist
const ALLOWED_COMMANDS: &[&str] = &["npx", "uvx"];

// 2. For npx/uvx, validate package names
fn validate_npx_args(args: &[String]) -> Result<(), AppError> {
    if args.is_empty() {
        return Err(AppError::bad_request("INVALID_ARGS", "npx requires package name"));
    }

    // First arg must be a valid package name (no command injection)
    let package = &args[0];
    if package.contains(';') || package.contains('&') || package.contains('|') {
        return Err(AppError::bad_request("INVALID_PACKAGE", "Invalid package name"));
    }

    // Allowlist known safe MCP server packages
    const ALLOWED_PACKAGES: &[&str] = &[
        "@modelcontextprotocol/server-filesystem",
        "mcp-server-fetch",
        "@browsermcp/mcp",
        "mcp-git-server",
    ];

    let package_name = package.trim_start_matches("-y").trim();
    if !ALLOWED_PACKAGES.iter().any(|&p| package_name == p) {
        return Err(AppError::bad_request(
            "PACKAGE_NOT_ALLOWED",
            format!("Package '{}' is not in the allowlist", package_name)
        ));
    }

    Ok(())
}
```

**Alternative:** Use a capability-based allowlist where each package name maps to allowed capabilities.

---

### 3. Environment Variable Blocklist Incomplete

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/client/stdio.rs`
**Lines:** 16-29, 74-89

**Vulnerability:**

The environment variable blocklist only covers database and API secrets, but misses many dangerous environment variables:

```rust
const BLOCKED_ENV_VARS: &[&str] = &[
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SECRET_KEY",
    "DATABASE_PASSWORD",
    "DB_PASSWORD",
    "PGPASSWORD",
    "MYSQL_PASSWORD",
    "REDIS_PASSWORD",
    "API_SECRET",
    "SECRET_KEY",
    "PRIVATE_KEY",
    "JWT_SECRET",
    "ENCRYPTION_KEY",
];
```

**Missing:**
- `PATH` - Can redirect commands to malicious binaries
- `LD_PRELOAD` / `DYLD_INSERT_LIBRARIES` - Code injection
- `HOME` - Can change where config files are read from
- `PYTHONPATH` / `NODE_PATH` - Module injection
- `SHELL` - Shell command injection
- Cloud provider credentials: `GOOGLE_APPLICATION_CREDENTIALS`, `AZURE_CLIENT_SECRET`
- Session tokens: `AWS_SESSION_TOKEN`, `GITHUB_TOKEN`, `GITLAB_TOKEN`

**Impact:**

Malicious admin creates MCP server with:
```json
{
  "environment_variables": {
    "LD_PRELOAD": "/tmp/malicious.so",
    "PATH": "/tmp/evil:/usr/bin"
  }
}
```

**Recommended Fix:**

```rust
const BLOCKED_ENV_VARS: &[&str] = &[
    // Existing secrets
    "AWS_SECRET_ACCESS_KEY", "AWS_SECRET_KEY", "AWS_SESSION_TOKEN",
    "DATABASE_PASSWORD", "DB_PASSWORD", "PGPASSWORD", "MYSQL_PASSWORD", "REDIS_PASSWORD",
    "API_SECRET", "SECRET_KEY", "PRIVATE_KEY", "JWT_SECRET", "ENCRYPTION_KEY",

    // Code injection
    "LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH",

    // Path manipulation
    "PATH", "PYTHONPATH", "NODE_PATH", "RUBYLIB", "PERL5LIB",
    "HOME", "TMPDIR", "TEMP", "TMP",

    // Shell manipulation
    "SHELL", "BASH_ENV", "ENV", "IFS",

    // Cloud credentials
    "GOOGLE_APPLICATION_CREDENTIALS", "GOOGLE_API_KEY",
    "AZURE_CLIENT_SECRET", "AZURE_CLIENT_ID",

    // Version control tokens
    "GITHUB_TOKEN", "GITLAB_TOKEN", "BITBUCKET_TOKEN",

    // CI/CD secrets
    "JENKINS_TOKEN", "CIRCLECI_TOKEN", "TRAVIS_TOKEN",
];

// Better approach: Use an allowlist instead
const ALLOWED_ENV_VARS: &[&str] = &[
    "LANG", "LC_ALL", "TZ",  // Locale/timezone
];

fn validate_env_var(key: &str) -> Result<(), AppError> {
    if !ALLOWED_ENV_VARS.contains(&key) {
        return Err(AppError::bad_request(
            "ENV_VAR_NOT_ALLOWED",
            format!("Environment variable '{}' is not allowed", key)
        ));
    }
    Ok(())
}
```

---

### 4. SSRF in HTTP/SSE Transport

**Severity:** CRITICAL
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/client/http.rs`
**Lines:** 23-61, `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/repository.rs`
**Lines:** 1209-1217

**Vulnerability:**

URL validation only checks if URL starts with `http://` or `https://`, but does NOT prevent SSRF attacks against internal services.

```rust
// repository.rs
fn validate_url(url: &str) -> Result<(), AppError> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError::bad_request(
            "INVALID_URL",
            "url must start with http:// or https://",
        ));
    }
    Ok(())  // ❌ No SSRF protection!
}
```

**Impact:**

Admin creates MCP server pointing to internal services:
```json
{
  "url": "http://169.254.169.254/latest/meta-data/iam/security-credentials/",
  "transport_type": "http"
}
```

**Attack Vectors:**
- `http://localhost:5432` - PostgreSQL database
- `http://127.0.0.1:6379` - Redis
- `http://169.254.169.254/` - AWS metadata service (credentials)
- `http://metadata.google.internal/` - GCP metadata
- `http://192.168.1.1/admin` - Internal admin panels
- `http://10.0.0.0/8` - Internal network scanning

**Recommended Fix:**

```rust
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

fn validate_url(url: &str) -> Result<(), AppError> {
    // Parse URL
    let parsed = url::Url::parse(url)
        .map_err(|e| AppError::bad_request("INVALID_URL", format!("Invalid URL: {}", e)))?;

    // Only allow http/https schemes
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(AppError::bad_request(
            "INVALID_SCHEME",
            "Only http:// and https:// schemes are allowed"
        ));
    }

    // Get host
    let host = parsed.host_str()
        .ok_or_else(|| AppError::bad_request("INVALID_URL", "URL must have a host"))?;

    // Block localhost and internal IPs
    if is_internal_host(host)? {
        return Err(AppError::bad_request(
            "SSRF_BLOCKED",
            "Cannot connect to internal/private IP addresses"
        ));
    }

    Ok(())
}

fn is_internal_host(host: &str) -> Result<bool, AppError> {
    // Check common localhost names
    if host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host.ends_with(".localhost") {
        return Ok(true);
    }

    // Check metadata services
    if host == "169.254.169.254"  // AWS
        || host == "metadata.google.internal"  // GCP
        || host == "169.254.169.254"  // Azure
    {
        return Ok(true);
    }

    // Try to resolve to IP and check if private
    match host.parse::<IpAddr>() {
        Ok(ip) => Ok(is_private_ip(&ip)),
        Err(_) => {
            // Hostname - would need DNS resolution to check
            // For security, consider blocking non-IP hosts or using DNS allowlist
            Ok(false)
        }
    }
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_private()
                || ipv4.is_loopback()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                // 169.254.0.0/16 - link-local
                || (ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254)
        },
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback() || ipv6.is_multicast()
        }
    }
}
```

**Additional Protection:** Configure HTTP client to disable redirect following or validate redirect targets.

---

### 5. Approval Forgery via Branch Manipulation

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/chat/extensions/mcp/approval/repository.rs`
**Lines:** 228-262

**Vulnerability:**

The `approve_tool_use()` function only checks that the approval is `pending` on the specified branch. It does NOT verify that the user approving is the same user who owns the conversation or has permission to approve.

```rust
pub async fn approve_tool_use(
    pool: &PgPool,
    tool_use_id: String,
    branch_id: Uuid,
    approved_by: Uuid,  // ❌ Not validated against conversation owner
    note: Option<String>,
) -> Result<ToolUseApproval, AppError> {
    let approval = sqlx::query_as!(
        ToolUseApproval,
        r#"
        UPDATE tool_use_approvals
        SET
            status = 'approved',
            approved_at = NOW(),
            approved_by = $3,
            approval_note = $4,
            updated_at = NOW()
        WHERE tool_use_id = $1 AND branch_id = $2 AND status = 'pending'
        -- ❌ No check that $3 (approved_by) matches conversation owner
        RETURNING ...
        "#,
        tool_use_id,
        branch_id,
        approved_by,
        note
    )
    .fetch_one(pool)
    .await?;

    Ok(approval)
}
```

**Impact:**

Attack scenario:
1. User A creates conversation with pending tool approval
2. User B discovers the branch_id (via URL manipulation or enumeration)
3. User B calls approval API with User A's branch_id
4. Tool is approved and executed in User A's context

**Exploitation:**

```bash
# User B intercepts or guesses branch_id from User A's conversation
curl -X POST /api/conversations/{conversation_id}/branches/{branch_id}/tool-approvals \
  -H "Authorization: Bearer USER_B_TOKEN" \
  -d '{"tool_approvals": [{"tool_use_id": "...", "decision": "approve"}]}'
```

**Recommended Fix:**

```rust
pub async fn approve_tool_use(
    pool: &PgPool,
    tool_use_id: String,
    branch_id: Uuid,
    approved_by: Uuid,
    note: Option<String>,
) -> Result<ToolUseApproval, AppError> {
    // First, verify the user owns the conversation
    let approval_check = sqlx::query!(
        r#"
        SELECT ta.user_id, c.user_id as conversation_owner
        FROM tool_use_approvals ta
        JOIN conversations c ON ta.conversation_id = c.id
        WHERE ta.tool_use_id = $1 AND ta.branch_id = $2 AND ta.status = 'pending'
        "#,
        tool_use_id,
        branch_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("Tool approval not found or already processed"))?;

    // Verify approved_by matches conversation owner
    if approval_check.conversation_owner != Some(approved_by) {
        return Err(AppError::forbidden(
            "UNAUTHORIZED_APPROVAL",
            "You can only approve tools in your own conversations"
        ));
    }

    // Now approve
    let approval = sqlx::query_as!(
        ToolUseApproval,
        r#"
        UPDATE tool_use_approvals
        SET
            status = 'approved',
            approved_at = NOW(),
            approved_by = $3,
            approval_note = $4,
            updated_at = NOW()
        WHERE tool_use_id = $1 AND branch_id = $2 AND status = 'pending'
        RETURNING ...
        "#,
        tool_use_id,
        branch_id,
        approved_by,
        note
    )
    .fetch_one(pool)
    .await?;

    Ok(approval)
}
```

**Also apply to `deny_tool_use()`** - same vulnerability exists there.

---

### 6. No Tool Argument Validation Against Schema

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/chat/extensions/mcp/helpers.rs`
**Lines:** 94-167

**Vulnerability:**

Tool arguments are passed directly from the LLM to the MCP server without validation against the tool's JSON schema. Malicious or malformed arguments could exploit vulnerabilities in MCP server implementations.

```rust
pub async fn execute_tool(
    session: &mut McpSession,
    tool_name: &str,
    input: Value,  // ❌ Not validated against schema!
    _server_name: &str,
    timeout_seconds: Option<i32>,
) -> McpContentData {
    // ...
    let result = tokio::time::timeout(
        timeout,
        session.call_tool(actual_tool_name, input.clone())  // ❌ Direct passthrough
    ).await;
```

**Impact:**

- Type confusion attacks (passing string where number expected)
- Injection attacks if MCP server doesn't validate
- Resource exhaustion (extremely large arguments)
- Schema bypass attacks

**Example Attack:**

MCP tool expects: `{"path": "/safe/directory/file.txt"}`
Attacker provides: `{"path": "../../../../etc/passwd"}`

If MCP server doesn't validate, path traversal succeeds.

**Recommended Fix:**

```rust
pub async fn execute_tool(
    session: &mut McpSession,
    tool_name: &str,
    input: Value,
    server_name: &str,
    timeout_seconds: Option<i32>,
) -> McpContentData {
    // 1. Get tool schema from session
    let tools = match session.list_tools().await {
        Ok(t) => t,
        Err(e) => {
            return McpContentData::ToolResult {
                tool_use_id: String::new(),
                name: Some(tool_name.to_string()),
                content: format!("Failed to get tool schema: {}", e),
                is_error: Some(true),
            };
        }
    };

    let actual_tool_name = if let Some(idx) = tool_name.rfind("__") {
        &tool_name[idx + 2..]
    } else {
        tool_name
    };

    let tool = tools.iter().find(|t| t.name == actual_tool_name);
    if tool.is_none() {
        return McpContentData::ToolResult {
            tool_use_id: String::new(),
            name: Some(tool_name.to_string()),
            content: format!("Tool '{}' not found", actual_tool_name),
            is_error: Some(true),
        };
    }

    // 2. Validate input against schema
    if let Err(e) = validate_against_schema(&input, &tool.unwrap().input_schema) {
        return McpContentData::ToolResult {
            tool_use_id: String::new(),
            name: Some(tool_name.to_string()),
            content: format!("Invalid arguments: {}", e),
            is_error: Some(true),
        };
    }

    // 3. Size limit check
    let input_size = serde_json::to_string(&input).unwrap_or_default().len();
    if input_size > 100_000 {  // 100KB limit
        return McpContentData::ToolResult {
            tool_use_id: String::new(),
            name: Some(tool_name.to_string()),
            content: format!("Arguments too large: {} bytes (max 100KB)", input_size),
            is_error: Some(true),
        };
    }

    // 4. Execute with timeout
    // ... existing code
}

fn validate_against_schema(input: &Value, schema: &Value) -> Result<(), String> {
    // Use jsonschema crate
    let compiled = jsonschema::JSONSchema::compile(schema)
        .map_err(|e| format!("Invalid schema: {}", e))?;

    compiled.validate(input)
        .map_err(|errors| {
            let error_messages: Vec<String> = errors
                .map(|e| e.to_string())
                .collect();
            error_messages.join("; ")
        })
}
```

Add to `Cargo.toml`:
```toml
jsonschema = "0.17"
```

---

### 7. SQL Injection in Group Assignment (Minor)

**Severity:** LOW
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/repository.rs`
**Lines:** 944-990

**Vulnerability:**

The code uses parameterized queries correctly in most places, but there's a pattern that could be problematic if refactored incorrectly. Currently safe, but worth noting for future changes.

**Current (Safe):**
```rust
pub async fn set_group_mcp_servers(
    pool: &PgPool,
    group_id: Uuid,
    server_ids: Vec<Uuid>,
) -> Result<(), AppError> {
    // ...
    for server_id in server_ids {
        sqlx::query!(
            "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id) VALUES ($1, $2)",
            group_id,
            server_id
        )
        .execute(&mut *tx)
        .await?;
    }
    // ...
}
```

This is safe because it uses parameterized queries. However, if someone tries to "optimize" this to a single query:

**Unsafe Pattern (DON'T DO THIS):**
```rust
let ids_str = server_ids.iter()
    .map(|id| format!("'{}'", id))
    .collect::<Vec<_>>()
    .join(",");
let query = format!(
    "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id) VALUES {}",
    ids_str
);
sqlx::query(&query).execute(pool).await?;
```

**Recommendation:**

Add code comment to prevent future SQL injection:
```rust
// SECURITY: Do not convert to string concatenation - keep parameterized queries
for server_id in server_ids {
    sqlx::query!(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id) VALUES ($1, $2)",
        group_id,
        server_id
    )
    .execute(&mut *tx)
    .await?;
}
```

---

### 8. No Audit Logging for Sensitive Operations

**Severity:** MEDIUM
**Files:** Multiple handler files

**Vulnerability:**

Critical operations (creating MCP servers, approving tool executions, modifying group assignments) have minimal or no audit logging. This makes incident response and forensics difficult.

**Missing Audit Logs:**

1. MCP server creation/modification
2. Tool approval/denial decisions
3. Group assignment changes
4. Tool execution (only basic tracing, not security events)
5. Failed authorization attempts

**Current Logging (Insufficient):**
```rust
// stdio.rs:104-109
tracing::info!(
    server_id = %self.server_id,
    server_name = %self.server_config.name,
    transport = "stdio",
    "MCP server connection initiated"
);
```

**Recommended Fix:**

Create a security audit log system:

```rust
// Security audit event types
#[derive(Debug, Serialize)]
enum SecurityAuditEvent {
    McpServerCreated {
        server_id: Uuid,
        server_name: String,
        transport_type: String,
        created_by: Uuid,
        is_system: bool,
    },
    McpServerModified {
        server_id: Uuid,
        modified_by: Uuid,
        changes: Vec<String>,
    },
    McpServerDeleted {
        server_id: Uuid,
        deleted_by: Uuid,
    },
    ToolExecutionAttempt {
        server_id: Uuid,
        server_name: String,
        tool_name: String,
        user_id: Uuid,
        conversation_id: Uuid,
        approved: bool,
        result: String,  // "success" | "error" | "unauthorized"
    },
    ToolApprovalDecision {
        tool_use_id: String,
        conversation_id: Uuid,
        decided_by: Uuid,
        decision: String,  // "approved" | "denied"
        tool_name: String,
        server_name: String,
    },
    GroupAssignmentModified {
        group_id: Uuid,
        modified_by: Uuid,
        servers_added: Vec<Uuid>,
        servers_removed: Vec<Uuid>,
    },
    UnauthorizedAccess {
        user_id: Uuid,
        server_id: Uuid,
        endpoint: String,
        reason: String,
    },
}

fn log_security_event(event: SecurityAuditEvent) {
    let event_json = serde_json::to_string(&event).unwrap_or_default();

    // Log to security audit log
    tracing::warn!(
        target: "security_audit",
        event = %event_json,
        "Security audit event"
    );

    // Could also write to dedicated audit log table
    // sqlx::query!("INSERT INTO security_audit_log ...").execute(pool).await;
}
```

**Usage:**
```rust
// In create_user_mcp_server
log_security_event(SecurityAuditEvent::McpServerCreated {
    server_id: server.id,
    server_name: server.name.clone(),
    transport_type: server.transport_type.to_string(),
    created_by: user_id,
    is_system: false,
});
```

---

### 9. No Rate Limiting on Tool Calls

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/handlers/runtime.rs`
**Lines:** 80-116

**Vulnerability:**

There is no rate limiting on tool calls. A malicious user or compromised account can execute unlimited tool calls, potentially:
- Exhausting server resources
- Running up costs on paid MCP services
- DOS attacking external MCP servers
- Triggering rate limits on downstream services

**Current Code (No Rate Limiting):**
```rust
pub async fn call_server_tool(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path((server_id, tool_name)): Path<(Uuid, String)>,
    Json(request): Json<CallToolRequest>,
) -> ApiResult<Json<CallToolResponse>> {
    // ❌ No rate limiting!

    // Check access...
    let session = session_manager.get_or_create(server_id).await?;
    let mut session = session.write().await;
    let result = session.call_tool(&tool_name, request.arguments).await?;
    // ...
}
```

**Attack Scenario:**
```bash
# Attacker runs loop
for i in {1..100000}; do
  curl -X POST /api/mcp/servers/{id}/tools/expensive_tool/call \
    -H "Authorization: Bearer $TOKEN" \
    -d '{"arguments": {}}' &
done
```

**Recommended Fix:**

Use a rate limiting middleware or implement per-user rate limits:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

struct RateLimiter {
    // user_id -> (count, window_start)
    limits: Arc<RwLock<HashMap<Uuid, (u32, Instant)>>>,
    max_calls: u32,
    window: Duration,
}

impl RateLimiter {
    fn new(max_calls: u32, window_secs: u64) -> Self {
        Self {
            limits: Arc::new(RwLock::new(HashMap::new())),
            max_calls,
            window: Duration::from_secs(window_secs),
        }
    }

    async fn check_limit(&self, user_id: Uuid) -> Result<(), AppError> {
        let mut limits = self.limits.write().await;
        let now = Instant::now();

        let entry = limits.entry(user_id).or_insert((0, now));

        // Reset window if expired
        if now.duration_since(entry.1) > self.window {
            *entry = (0, now);
        }

        // Check limit
        if entry.0 >= self.max_calls {
            return Err(AppError::too_many_requests(
                "RATE_LIMIT_EXCEEDED",
                format!("Maximum {} calls per {} seconds", self.max_calls, self.window.as_secs())
            ));
        }

        // Increment counter
        entry.0 += 1;

        Ok(())
    }
}

// In handler
pub async fn call_server_tool(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Extension(rate_limiter): Extension<Arc<RateLimiter>>,
    Path((server_id, tool_name)): Path<(Uuid, String)>,
    Json(request): Json<CallToolRequest>,
) -> ApiResult<Json<CallToolResponse>> {
    // Check rate limit
    rate_limiter.check_limit(auth.user.id).await?;

    // ... rest of handler
}
```

**Suggested Limits:**
- 100 tool calls per user per minute
- 1000 tool calls per user per hour
- Configurable per group or role

---

### 10. Timeout Not Enforced at HTTP Client Level

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/client/http.rs`
**Lines:** 31-34, 93-96

**Vulnerability:**

While the HTTP client sets a timeout, there's no timeout on individual request operations like waiting for response body. A malicious MCP server could keep the connection open indefinitely by slowly sending data.

**Current Code:**
```rust
let mut client_builder = Client::builder()
    .timeout(std::time::Duration::from_secs(
        server.timeout_seconds.max(1) as u64
    ));  // ❌ Only overall timeout, not per-operation

// ...

let response = request.send()
    .await
    .map_err(|e| AppError::internal_error(format!("HTTP request failed: {}", e)))?;

// ❌ No timeout on reading response body
let response_text = response.text().await
    .map_err(|e| AppError::internal_error(format!("Failed to get response text: {}", e)))?;
```

**Attack:**
Malicious MCP server sends 1 byte per second indefinitely.

**Recommended Fix:**

```rust
use tokio::time::timeout;

async fn request<T: serde::de::DeserializeOwned>(
    &self,
    method: &str,
    params: Value,
) -> Result<T, AppError> {
    let request_timeout = Duration::from_secs(
        self.server_config.timeout_seconds.max(1) as u64
    );

    // Wrap entire request in timeout
    let result = timeout(request_timeout, async {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let url = if self.base_url.ends_with('/') {
            format!("{}mcp", self.base_url)
        } else {
            format!("{}/mcp", self.base_url)
        };

        let mut request = self.client
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .json(&request_body);

        if let Ok(session_guard) = self.session_id.read() {
            if let Some(ref session_id) = *session_guard {
                request = request.header("mcp-session-id", session_id);
            }
        }

        let response = request.send().await
            .map_err(|e| AppError::internal_error(format!("HTTP request failed: {}", e)))?;

        // Extract session ID
        if let Some(session_id) = response.headers().get("mcp-session-id") {
            if let Ok(session_str) = session_id.to_str() {
                if let Ok(mut session_guard) = self.session_id.write() {
                    *session_guard = Some(session_str.to_string());
                }
            }
        }

        // Read response with size limit
        let response_text = Self::read_response_with_limit(response, 10_000_000).await?;  // 10MB limit

        // ... rest of parsing
        Ok(result)
    }).await;

    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(AppError::internal_error(
            format!("Request timed out after {}s", request_timeout.as_secs())
        )),
    }
}

async fn read_response_with_limit(
    response: reqwest::Response,
    max_size: usize,
) -> Result<String, AppError> {
    use futures::StreamExt;

    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|e| AppError::internal_error(format!("Failed to read chunk: {}", e)))?;

        bytes.extend_from_slice(&chunk);

        if bytes.len() > max_size {
            return Err(AppError::internal_error(
                format!("Response too large (max {}MB)", max_size / 1_000_000)
            ));
        }
    }

    String::from_utf8(bytes)
        .map_err(|e| AppError::internal_error(format!("Invalid UTF-8 in response: {}", e)))
}
```

---

### 11. Custom Headers Allow Header Injection

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/client/http.rs`
**Lines:** 37-50

**Vulnerability:**

Custom headers from the database are added to HTTP requests without validation. A malicious admin could inject headers that:
- Override authentication headers
- Cause CRLF injection
- Bypass security controls on the MCP server

**Current Code:**
```rust
// Add custom headers if provided
if let Some(headers_map) = server.headers.as_object() {
    let mut headers = reqwest::header::HeaderMap::new();
    for (key, value) in headers_map {
        if let Some(val_str) = value.as_str() {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(val_str)  // ❌ No validation
            ) {
                headers.insert(name, val);  // ❌ Can override any header
            }
        }
    }
    client_builder = client_builder.default_headers(headers);
}
```

**Attack Scenarios:**

1. **Override Authorization:**
```json
{
  "headers": {
    "Authorization": "Bearer attacker_token"
  }
}
```

2. **CRLF Injection:**
```json
{
  "headers": {
    "X-Custom": "value\r\nX-Admin: true"
  }
}
```

**Recommended Fix:**

```rust
// Blocklist of dangerous headers
const BLOCKED_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "host",
    "connection",
    "upgrade",
    "transfer-encoding",
    "content-length",
];

fn validate_custom_headers(headers_map: &serde_json::Map<String, Value>) -> Result<(), AppError> {
    for (key, value) in headers_map {
        let key_lower = key.to_lowercase();

        // Block dangerous headers
        if BLOCKED_HEADERS.contains(&key_lower.as_str()) {
            return Err(AppError::bad_request(
                "BLOCKED_HEADER",
                format!("Header '{}' is not allowed", key)
            ));
        }

        // Validate header name
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(AppError::bad_request(
                "INVALID_HEADER_NAME",
                format!("Invalid header name: {}", key)
            ));
        }

        // Validate header value (no CRLF)
        if let Some(val_str) = value.as_str() {
            if val_str.contains('\r') || val_str.contains('\n') {
                return Err(AppError::bad_request(
                    "INVALID_HEADER_VALUE",
                    format!("Header value contains CRLF: {}", key)
                ));
            }

            // Length limit
            if val_str.len() > 1000 {
                return Err(AppError::bad_request(
                    "HEADER_TOO_LONG",
                    format!("Header value too long: {}", key)
                ));
            }
        }
    }

    Ok(())
}

// In HttpMcpClient::new()
if let Some(headers_map) = server.headers.as_object() {
    validate_custom_headers(headers_map)?;  // Validate first

    let mut headers = reqwest::header::HeaderMap::new();
    for (key, value) in headers_map {
        if let Some(val_str) = value.as_str() {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(val_str)
            ) {
                headers.insert(name, val);
            }
        }
    }
    client_builder = client_builder.default_headers(headers);
}
```

---

### 12. No Protection Against Session Hijacking

**Severity:** LOW
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/client/manager.rs`
**Lines:** 25-56

**Vulnerability:**

MCP sessions are stored in a global HashMap keyed by `server_id` only. If User A creates a session to Server X, User B (who also has access to Server X) will reuse the same session. This could lead to:
- Session state confusion
- One user's actions affecting another user's session
- Information leakage if sessions maintain user-specific state

**Current Code:**
```rust
pub struct McpSessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<McpSession>>>>>,  // ❌ Only keyed by server_id
    pool: PgPool,
}

pub async fn get_or_create(
    &self,
    server_id: Uuid,
) -> Result<Arc<RwLock<McpSession>>, AppError> {
    // Check if session exists
    {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(&server_id) {
            return Ok(session.clone());  // ❌ Shared across all users!
        }
    }
    // ...
}
```

**Impact:**

If MCP protocol supports user-specific session state (which it might in the future), sessions could leak information between users.

**Recommended Fix:**

Change session key to include user_id:

```rust
pub struct McpSessionManager {
    // Key: (server_id, user_id)
    sessions: Arc<RwLock<HashMap<(Uuid, Uuid), Arc<RwLock<McpSession>>>>>,
    pool: PgPool,
}

pub async fn get_or_create(
    &self,
    server_id: Uuid,
    user_id: Uuid,  // Add user_id parameter
) -> Result<Arc<RwLock<McpSession>>, AppError> {
    let session_key = (server_id, user_id);

    // Check if session exists
    {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(&session_key) {
            return Ok(session.clone());
        }
    }

    // Create new session
    let repo = McpRepository::new(self.pool.clone());

    // Verify user access
    if !repo.can_user_access_server(user_id, server_id).await? {
        return Err(AppError::forbidden(
            "USER_NO_ACCESS",
            "You do not have access to this server"
        ));
    }

    let server = repo.get_system_server(server_id).await?
        .ok_or_else(|| AppError::not_found("Server not found"))?;

    if !server.enabled {
        return Err(AppError::bad_request("server_disabled", "Server is disabled"));
    }

    let session = McpSession::new(server).await?;
    let session = Arc::new(RwLock::new(session));

    // Store session with user-specific key
    let mut sessions = self.sessions.write().await;
    sessions.insert(session_key, session.clone());

    Ok(session)
}

pub async fn close(&self, server_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let session = {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&(server_id, user_id))
    };

    if let Some(session) = session {
        let mut session = session.write().await;
        session.disconnect().await?;
    }

    Ok(())
}
```

**Update `cleanup_idle()`** to iterate over `(server_id, user_id)` tuples.

---

## Additional Recommendations

### 13. Content Security

**Issue:** Tool result content is truncated to 100KB but not sanitized for malicious content.

**Recommendation:**
```rust
fn sanitize_tool_result(content: &str) -> String {
    // Remove potential XSS if content is rendered in UI
    // Remove ANSI escape codes
    // Remove control characters
    content
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}
```

### 14. Monitoring and Alerting

**Implement alerts for:**
- Unusually high tool execution rates
- Repeated authorization failures
- Large tool result payloads
- Tool execution errors above threshold
- MCP server connection failures

### 15. Defense in Depth

**Additional layers:**
- Network isolation for MCP server processes (containers/VMs)
- Separate OS user accounts for MCP server execution
- SELinux/AppArmor profiles for stdio transport
- Allowlist of allowed file paths for filesystem MCP servers

---

## Summary of Recommendations by Priority

### CRITICAL (Fix Immediately)

1. **Finding #1:** Add authorization check to `McpSessionManager::get_or_create()`
2. **Finding #4:** Implement SSRF protection in URL validation

### HIGH (Fix Within 1 Week)

3. **Finding #2:** Restrict command allowlist and validate args
4. **Finding #5:** Verify conversation ownership in approval functions
5. **Finding #6:** Validate tool arguments against JSON schema
6. **Finding #9:** Implement rate limiting on tool calls

### MEDIUM (Fix Within 1 Month)

7. **Finding #3:** Expand environment variable blocklist
8. **Finding #8:** Add comprehensive audit logging
9. **Finding #10:** Enforce timeouts at all HTTP client levels
10. **Finding #11:** Validate and restrict custom headers

### LOW (Monitor/Consider)

11. **Finding #7:** Add SQL injection prevention comments
12. **Finding #12:** Use per-user session keys

---

## Testing Recommendations

### Security Test Cases

1. **Authorization Tests:**
   - User A attempts to use MCP server assigned only to Group B
   - User A attempts to approve tool in User B's conversation
   - Admin attempts to assign user server to group

2. **SSRF Tests:**
   - Create MCP server with `url: "http://localhost:5432"`
   - Create MCP server with `url: "http://169.254.169.254/"`
   - Create MCP server with `url: "http://[::1]/admin"`

3. **Command Injection Tests:**
   - System server with `command: "python3", args: ["-c", "import os; os.system('id')"]`
   - System server with `args: ["../../etc/passwd"]`

4. **Rate Limit Tests:**
   - Send 200 tool call requests in 10 seconds
   - Verify 429 Too Many Requests returned

5. **Input Validation Tests:**
   - Tool call with arguments larger than 100KB
   - Tool call with arguments not matching schema
   - Tool call with null/undefined required fields

### Penetration Testing

Consider hiring external security auditors to:
- Perform black-box testing of MCP endpoints
- Attempt privilege escalation via MCP servers
- Test for additional SSRF/RCE vulnerabilities

---

## Compliance Considerations

If this application handles regulated data:

- **GDPR:** MCP tool execution logs may contain personal data - ensure proper retention/deletion
- **SOC 2:** Audit logging is required for all security-relevant actions
- **PCI DSS:** If payment data flows through MCP servers, additional controls needed
- **HIPAA:** MCP servers accessing PHI must be audited and access-controlled

---

## Conclusion

The MCP module implements a powerful but high-risk feature: executing arbitrary external tools. While the current implementation has basic security controls, multiple critical vulnerabilities exist that could lead to unauthorized access, command injection, SSRF, and privilege escalation.

**Priority actions:**
1. Fix authorization bypass in session manager (Finding #1)
2. Implement SSRF protection (Finding #4)
3. Add rate limiting (Finding #9)
4. Implement comprehensive audit logging (Finding #8)

Once these are addressed, conduct a follow-up security review and penetration test before enabling MCP in production.

---

**Report prepared by:** Claude (Sonnet 4.5)
**Date:** 2025-01-21
**Next review:** After critical fixes are implemented
