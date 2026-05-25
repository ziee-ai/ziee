# Security Audit Report: Assistant & Hub Modules

**Audit Date:** 2025-01-21
**Auditor:** Security Analysis
**Scope:** Assistant, Hub, App, Hardware, and Health modules

---

## Executive Summary

This audit examined the assistant and hub modules in the Ziee Chat application, focusing on authorization, input validation, SQL injection, information disclosure, and other security concerns. The audit identified **10 security issues** ranging from MEDIUM to LOW severity.

### Critical Findings Summary
- **0 CRITICAL** issues
- **0 HIGH** issues
- **5 MEDIUM** issues
- **5 LOW** issues

**Overall Assessment:** The codebase demonstrates good security practices with parameterized queries and proper ownership checks. However, several areas require attention around input validation, information disclosure, and path traversal prevention.

---

## Findings

### 1. Path Traversal Vulnerability in Hub Locale Loading

**Severity:** MEDIUM
**Module:** Hub
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/hub/hub_manager.rs`
**Lines:** 110-150

**Description:**
The `load_hub_data_with_locale()` function accepts user-controlled locale strings without validation and uses them to construct file paths. This could allow directory traversal attacks.

```rust
pub async fn load_hub_data_with_locale(&self, locale: &str) -> Result<HubData, AppError> {
    let hub_dir = self.app_data_dir.join("hub");
    let version = self.get_current_version("llm-models").await?;

    // ...

    // If locale is not English, merge with locale-specific overrides
    let (models, assistants, mcp_servers) = if locale != "en" {
        let models_override: Option<Vec<serde_json::Value>> = self
            .load_json_file_optional(
                hub_dir
                    .join("llm-models")
                    .join(&version)
                    .join(format!("{}.json", locale)),  // ⚠️ Unsanitized user input
            )
            .await?;
```

**Attack Scenario:**
```
GET /api/hub/models?lang=../../../etc/passwd
```

**Impact:**
- Read arbitrary files on the system
- Information disclosure
- Potential data exfiltration

**Recommended Fix:**
```rust
// Add to hub/types.rs
fn validate_locale(locale: &str) -> Result<(), AppError> {
    // Only allow alphanumeric and hyphen (e.g., en, es, zh-CN)
    if !locale.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(AppError::bad_request(
            "INVALID_LOCALE",
            "Locale must contain only alphanumeric characters and hyphens"
        ));
    }

    // Limit length
    if locale.len() > 10 {
        return Err(AppError::bad_request(
            "INVALID_LOCALE",
            "Locale must be 10 characters or less"
        ));
    }

    Ok(())
}

// Use in handlers.rs before calling load_hub_data_with_locale
validate_locale(&query.lang)?;
```

---

### 2. Insufficient Validation of Assistant Instructions (Prompt Injection Risk)

**Severity:** MEDIUM
**Module:** Assistant
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/assistant/handlers.rs`
**Lines:** 52-74, 284-306

**Description:**
Assistant instructions/system prompts are accepted without length limits or content validation. Malicious users could inject extremely long or malicious prompts that could:
- Cause token exhaustion in downstream AI models
- Inject malicious instructions to manipulate AI behavior
- Cause denial of service through resource exhaustion

```rust
pub async fn create_user_assistant(
    auth: RequirePermissions<(AssistantsCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(mut request): Json<CreateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    // Validate name is not empty
    if request.name.trim().is_empty() {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "Assistant name cannot be empty").into(),
        );
    }
    // ⚠️ No validation on instructions field
    // ⚠️ No length limits on instructions, description, or parameters
```

**Attack Scenario:**
```json
POST /api/assistants
{
  "name": "Evil Assistant",
  "instructions": "<10MB of malicious prompt engineering instructions>"
}
```

**Impact:**
- Prompt injection attacks against AI models
- Resource exhaustion
- Token/cost abuse
- Manipulation of AI behavior

**Recommended Fix:**
```rust
// Add validation function
fn validate_assistant_content(request: &CreateAssistantRequest) -> Result<(), AppError> {
    const MAX_INSTRUCTIONS_LENGTH: usize = 50_000; // ~12k tokens
    const MAX_DESCRIPTION_LENGTH: usize = 2_000;
    const MAX_NAME_LENGTH: usize = 255;

    if request.name.len() > MAX_NAME_LENGTH {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Name must be 255 characters or less"
        ));
    }

    if let Some(ref instructions) = request.instructions {
        if instructions.len() > MAX_INSTRUCTIONS_LENGTH {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "Instructions must be 50,000 characters or less"
            ));
        }
    }

    if let Some(ref description) = request.description {
        if description.len() > MAX_DESCRIPTION_LENGTH {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "Description must be 2,000 characters or less"
            ));
        }
    }

    Ok(())
}

// Add to handler before creating assistant
validate_assistant_content(&request)?;
```

---

### 3. No Rate Limiting on Hub Refresh Operations

**Severity:** MEDIUM
**Module:** Hub
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/hub/handlers.rs`
**Lines:** 150-235

**Description:**
The hub refresh endpoints (`/hub/models/refresh`, `/hub/assistants/refresh`, `/hub/mcp-servers/refresh`) make external HTTP requests to GitHub without rate limiting. An attacker with valid credentials could abuse these endpoints to:
- Cause denial of service through repeated GitHub requests
- Trigger IP-based rate limiting from GitHub
- Consume bandwidth and system resources

```rust
pub async fn refresh_hub_models(
    _auth: RequirePermissions<(HubModelsRefresh,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.get_current_version("llm-models").await?;
    hub_manager.refresh_hub_category("llm-models").await?;  // ⚠️ No rate limiting
```

**Impact:**
- Denial of service
- GitHub API rate limit exhaustion
- IP blacklisting
- Bandwidth abuse

**Recommended Fix:**
```rust
// Add rate limiting middleware or use a rate limiter
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

// Global rate limiter (1 refresh per minute per category)
static HUB_REFRESH_LIMITER: Lazy<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> =
    Lazy::new(|| {
        RateLimiter::direct(Quota::per_minute(NonZeroU32::new(1).unwrap()))
    });

// In handler
if HUB_REFRESH_LIMITER.check().is_err() {
    return Err((
        StatusCode::TOO_MANY_REQUESTS,
        AppError::too_many_requests(
            "RATE_LIMIT_EXCEEDED",
            "Hub refresh can only be called once per minute"
        )
    ));
}
```

---

### 4. Information Disclosure in Hardware Endpoint

**Severity:** MEDIUM
**Module:** Hardware
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/hardware/handlers.rs`
**Lines:** 28-80

**Description:**
The hardware endpoint exposes detailed system information including OS version, kernel version, CPU model, architecture, memory capacity, and GPU details. This information could be used for:
- Fingerprinting the system
- Identifying vulnerable software versions
- Planning targeted attacks

```rust
pub async fn get_hardware_info(
    _auth: RequirePermissions<(HardwareRead,)>,
) -> ApiResult<Json<HardwareInfoResponse>> {
    // ...
    let operating_system = OperatingSystemInfo {
        name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),  // ⚠️
        kernel_version: System::kernel_version(),  // ⚠️
        architecture: std::env::consts::ARCH.to_string(),  // ⚠️
    };
```

**Impact:**
- Information disclosure
- System fingerprinting
- Reconnaissance for targeted attacks

**Recommended Fix:**
```rust
// Option 1: Reduce information granularity
let operating_system = OperatingSystemInfo {
    name: System::name().unwrap_or_else(|| "Unknown".to_string()),
    version: None,  // Don't expose exact version
    kernel_version: None,  // Don't expose kernel version
    architecture: std::env::consts::ARCH.to_string(),  // Generic (x86_64, aarch64)
};

// Option 2: Add admin-only permission for detailed info
if !auth.user.is_admin {
    // Return minimal info for non-admins
}

// Option 3: Document in security policy that this endpoint requires trust
// and should only be exposed to authenticated, trusted users
```

**Note:** This may be acceptable depending on deployment model (single-user vs multi-tenant). Document security assumptions clearly.

---

### 5. Weak Password Requirements

**Severity:** MEDIUM
**Module:** App
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/app/utils.rs`
**Lines:** 75-77

**Description:**
The password strength validation only checks for a minimum of 8 characters without requiring complexity (uppercase, lowercase, numbers, special characters).

```rust
pub fn is_strong_password(password: &str) -> bool {
    password.len() >= 8  // ⚠️ Only length check
}
```

**Impact:**
- Weak passwords like "password" or "12345678" would be accepted
- Increased risk of brute force attacks
- Credential stuffing attacks more likely to succeed

**Recommended Fix:**
```rust
pub fn is_strong_password(password: &str) -> bool {
    const MIN_LENGTH: usize = 12;  // Increase to 12

    if password.len() < MIN_LENGTH {
        return false;
    }

    // Check for at least 3 of 4 character types
    let has_lowercase = password.chars().any(|c| c.is_lowercase());
    let has_uppercase = password.chars().any(|c| c.is_uppercase());
    let has_digit = password.chars().any(|c| c.is_numeric());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    let complexity_count = [has_lowercase, has_uppercase, has_digit, has_special]
        .iter()
        .filter(|&&x| x)
        .count();

    complexity_count >= 3
}
```

---

### 6. No Input Sanitization for Hub Data

**Severity:** LOW
**Module:** Hub
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/hub/handlers.rs`
**Lines:** 241-306, 312-395, 401-537

**Description:**
When creating entities from hub data (assistants, MCP servers, models), the data from external JSON files is used directly without sanitization or validation. If hub data is compromised, malicious content could be injected.

```rust
pub async fn create_assistant_from_hub(
    auth: RequirePermissions<(HubAssistantsCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateAssistantFromHubRequest>,
) -> ApiResult<Json<AssistantFromHubResponse>> {
    // 1. Load hub assistant
    let hub_data = hub_manager.load_hub_data_with_locale("en").await?;

    let hub_assistant = hub_data
        .assistants
        .into_iter()
        .find(|a| a.id == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub assistant '{}'", request.hub_id)))?;

    // 2. Build create assistant request
    let create_request = crate::modules::assistant::types::CreateAssistantRequest {
        name: request.name.unwrap_or(hub_assistant.name.clone()),  // ⚠️ No sanitization
        description: request.description.or(hub_assistant.description.clone()),  // ⚠️
        instructions: request.instructions.or(hub_assistant.instructions.clone()),  // ⚠️
```

**Impact:**
- If hub data source is compromised, malicious content propagates to database
- XSS risks if data is rendered in frontend
- Potential for injection attacks

**Recommended Fix:**
```rust
fn sanitize_hub_text(text: &str, max_length: usize) -> Result<String, AppError> {
    // Limit length
    if text.len() > max_length {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Hub data exceeds maximum length"
        ));
    }

    // Strip null bytes
    let cleaned = text.replace('\0', "");

    // Optionally: HTML-escape or strip dangerous characters
    Ok(cleaned)
}

// Apply to all hub data before use
let name = sanitize_hub_text(&hub_assistant.name, 255)?;
let description = hub_assistant.description
    .as_ref()
    .map(|d| sanitize_hub_text(d, 2000))
    .transpose()?;
```

---

### 7. Missing Pagination Limits

**Severity:** LOW
**Module:** Assistant
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/assistant/handlers.rs`
**Lines:** 29-45

**Description:**
The pagination query parameters accept any `i64` value without upper bounds. An attacker could request extremely large page sizes to cause memory exhaustion.

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PaginationQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: i64,  // ⚠️ No upper limit

    /// Items per page
    #[serde(default = "default_limit")]
    pub limit: i64,  // ⚠️ No upper limit
}

fn default_page() -> i64 {
    1
}
fn default_limit() -> i64 {
    20
}
```

**Attack Scenario:**
```
GET /api/assistants?limit=999999999
```

**Impact:**
- Memory exhaustion
- Denial of service
- Database performance degradation

**Recommended Fix:**
```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PaginationQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: i64,

    /// Items per page
    #[serde(default = "default_limit")]
    pub limit: i64,
}

impl PaginationQuery {
    const MAX_LIMIT: i64 = 100;
    const MAX_PAGE: i64 = 10_000;

    pub fn validate(&mut self) -> Result<(), AppError> {
        if self.page < 1 {
            self.page = 1;
        }
        if self.page > Self::MAX_PAGE {
            return Err(AppError::bad_request(
                "INVALID_PAGINATION",
                "Page number too large"
            ));
        }

        if self.limit < 1 {
            self.limit = default_limit();
        }
        if self.limit > Self::MAX_LIMIT {
            self.limit = Self::MAX_LIMIT;
        }

        Ok(())
    }
}

// In handlers
let mut query = query;
query.validate()?;
```

---

### 8. Template Assistant Enumeration

**Severity:** LOW
**Module:** Assistant
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/assistant/handlers.rs`
**Lines:** 319-347

**Description:**
Any authenticated user can list all template assistants, potentially revealing sensitive system prompts or organizational strategies embedded in template instructions.

```rust
pub async fn list_template_assistants(
    _auth: RequirePermissions<(AssistantsTemplateRead,)>,  // ⚠️ Any user with this permission
    Query(query): Query<PaginationQuery>,
) -> ApiResult<Json<AssistantListResponse>> {
    let response = Repos
        .assistant
        .list(
            None, // No user filter for templates
            true, // Only templates
            query.page,
            query.limit,
        )
        .await?;
```

**Impact:**
- Information disclosure of system prompts
- Intellectual property exposure
- Reconnaissance for prompt injection attacks

**Recommended Fix:**
Consider whether template assistants should be:
1. **Public by design** - If so, document this is expected behavior
2. **Admin-only** - Change permission to admin-only access
3. **Filtered** - Only show enabled templates to non-admins

```rust
// Option: Add filtering based on user role
pub async fn list_template_assistants(
    auth: RequirePermissions<(AssistantsTemplateRead,)>,
    Query(query): Query<PaginationQuery>,
) -> ApiResult<Json<AssistantListResponse>> {
    let show_all = auth.user.is_admin;

    // If not admin, only show enabled templates
    // This would require modifying the repository function
    let response = Repos
        .assistant
        .list_templates(query.page, query.limit, show_all)
        .await?;
```

---

### 9. Hub GitHub URL Hardcoded

**Severity:** LOW
**Module:** Hub
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/hub/hub_manager.rs`
**Lines:** 10

**Description:**
The GitHub repository URL is hardcoded and appears to be a placeholder. If this goes to production without change, it could:
- Cause runtime errors (404s)
- Create a security risk if an attacker registers the placeholder organization

```rust
const GITHUB_HUB_REPO: &str = "https://raw.githubusercontent.com/YOUR_ORG/ziee-hub/main";
// ⚠️ Placeholder URL
```

**Impact:**
- Broken functionality in production
- Potential for malicious takeover of placeholder namespace

**Recommended Fix:**
```rust
// Use environment variable or configuration
fn get_hub_repo_url() -> String {
    std::env::var("HUB_REPOSITORY_URL")
        .unwrap_or_else(|_| {
            // Fail loudly if not configured
            panic!("HUB_REPOSITORY_URL environment variable must be set")
        })
}

// Or load from config file
const GITHUB_HUB_REPO: &str = env!("HUB_REPOSITORY_URL",
    "HUB_REPOSITORY_URL must be set at compile time");
```

---

### 10. Health Check Exposes Service Status

**Severity:** LOW
**Module:** Health
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/health/handlers.rs`
**Lines:** 12-22

**Description:**
The health check endpoint is unauthenticated and always returns "ok". While minimal, it confirms the service is running and could be used for:
- Service enumeration
- Timing attacks to infer system load
- DDoS amplification

```rust
pub async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),  // ⚠️ Always returns ok
        }),
    )
}
```

**Impact:**
- Service enumeration
- Limited information disclosure

**Recommended Fix:**
This is typically acceptable for health checks. If concerned:

```rust
// Option 1: Add minimal obfuscation
pub async fn health_check() -> StatusCode {
    StatusCode::NO_CONTENT  // Return 204 with no body
}

// Option 2: Add actual health checks
pub async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    // Check database connectivity
    let db_healthy = Repos.health_check().await.is_ok();

    if db_healthy {
        (StatusCode::OK, Json(HealthResponse { status: "ok".to_string() }))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(HealthResponse {
            status: "degraded".to_string()
        }))
    }
}
```

---

## Positive Security Observations

### 1. Excellent SQL Injection Prevention
All database queries use SQLx's compile-time checked parameterized queries (`sqlx::query!` macro), which prevents SQL injection attacks:

```rust
// Example from assistant/repository.rs
sqlx::query!(
    "UPDATE assistants SET is_default = false WHERE created_by = $1 AND is_template = false",
    uid
)
```

### 2. Strong Ownership Validation
User assistant handlers properly validate ownership before allowing operations:

```rust
// Check ownership
if assistant.created_by != Some(auth.user.id) {
    return Err(AppError::forbidden(
        "ACCESS_DENIED",
        "You can only access your own assistants",
    )
    .into());
}
```

### 3. Proper Permission Checks
All endpoints use the `RequirePermissions` extractor to enforce RBAC:

```rust
pub async fn create_user_assistant(
    auth: RequirePermissions<(AssistantsCreate,)>,
    // ...
)
```

### 4. Transaction Safety
Critical operations use database transactions to prevent race conditions:

```rust
// Start a transaction to handle default assistant logic
let mut tx = pool.begin().await.map_err(AppError::database_error)?;

// ... multiple operations ...

// Commit the transaction
tx.commit().await.map_err(AppError::database_error)?;
```

### 5. Separate User and Template Namespaces
Clear separation between user assistants and template assistants prevents privilege escalation:

```rust
// Force is_template to false for user assistants
request.is_template = Some(false);

// Force is_template to true for template assistants
request.is_template = Some(true);
```

---

## Recommendations Summary

### Immediate Actions (MEDIUM Severity)

1. **Add locale validation** - Implement whitelist validation for locale parameter to prevent path traversal
2. **Add assistant content validation** - Implement length limits on instructions, descriptions, and names
3. **Implement rate limiting** - Add rate limits to hub refresh endpoints
4. **Review hardware information disclosure** - Decide if detailed system info should be admin-only
5. **Strengthen password requirements** - Increase minimum length and add complexity requirements

### Short-term Actions (LOW Severity)

6. **Sanitize hub data** - Add validation/sanitization for data loaded from hub files
7. **Add pagination limits** - Cap maximum page size and page number
8. **Review template access** - Decide if template assistant listing should be restricted
9. **Configure hub URL** - Replace hardcoded placeholder with configuration
10. **Enhance health check** - Add actual health checks for critical dependencies

### Long-term Improvements

1. **Add request logging** - Log all assistant creation/modification for audit trail
2. **Implement content scanning** - Scan assistant instructions for known malicious patterns
3. **Add metrics** - Monitor hub refresh frequency and hardware endpoint usage
4. **Security documentation** - Document security assumptions and deployment requirements
5. **Penetration testing** - Conduct regular security testing of these endpoints

---

## Compliance Notes

### OWASP Top 10 2021 Mapping

- **A01:2021 – Broken Access Control** - ✅ Good (ownership checks, RBAC)
- **A02:2021 – Cryptographic Failures** - N/A
- **A03:2021 – Injection** - ✅ Excellent (parameterized queries)
- **A04:2021 – Insecure Design** - ⚠️ Medium (hub refresh rate limiting needed)
- **A05:2021 – Security Misconfiguration** - ⚠️ Medium (weak passwords, info disclosure)
- **A06:2021 – Vulnerable Components** - N/A (requires dependency audit)
- **A07:2021 – Identification and Authentication** - ⚠️ Medium (weak password policy)
- **A08:2021 – Software and Data Integrity** - ⚠️ Low (hub data validation)
- **A09:2021 – Security Logging and Monitoring** - ℹ️ Info (could be improved)
- **A10:2021 – Server-Side Request Forgery (SSRF)** - ⚠️ Low (hub refresh to GitHub)

---

## Conclusion

The assistant and hub modules demonstrate solid security fundamentals with excellent SQL injection prevention and proper access controls. The identified issues are primarily related to input validation and information disclosure, which should be addressed to further strengthen the security posture.

The codebase would benefit from:
1. Comprehensive input validation on all user-supplied data
2. Rate limiting on resource-intensive operations
3. Careful consideration of information disclosure in system endpoints
4. Enhanced password complexity requirements

Overall security rating: **B+ (Good with room for improvement)**

---

## References

- SQLx Security: https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-do-a-select--from-query
- OWASP Input Validation: https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html
- OWASP Path Traversal: https://owasp.org/www-community/attacks/Path_Traversal
- OWASP Password Storage: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html

---

**End of Report**
