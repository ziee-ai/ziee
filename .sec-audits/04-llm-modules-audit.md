# LLM Modules Security Audit

**Audit Date:** 2025-01-21
**Audited By:** Security Review System
**Scope:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_*`

## Executive Summary

This audit reviewed four LLM-related modules for security vulnerabilities:
- `llm_model/` - Model file management and downloads
- `llm_provider/` - Provider configuration and API keys
- `llm_provider_files/` - Provider file attachment handling
- `llm_repository/` - Model repository management

**Overall Security Posture:** MODERATE RISK

**Critical Issues:** 1
**High Issues:** 4
**Medium Issues:** 6
**Low Issues:** 3

The codebase demonstrates good security practices in several areas (SQL injection prevention, permission-based access control), but has concerning issues around sensitive data exposure, insufficient input validation, and SSRF vulnerabilities.

---

## Critical Findings

### CRIT-1: API Keys Exposed in API Responses

**Severity:** CRITICAL
**Module:** `llm_provider`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/models.rs:34-35`

**Issue:**
Provider API keys are included in all API responses without filtering. This exposes sensitive credentials to any authenticated user with read permissions.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProvider {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,  // ❌ CRITICAL: API key exposed in responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub built_in: bool,
    pub proxy_settings: ProxySettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Affected Endpoints:**
- `GET /api/llm-providers` (list_providers)
- `GET /api/llm-providers/{provider_id}` (get_provider)
- `GET /api/llm-providers/{provider_id}/groups` (get_provider_groups)
- `GET /api/groups/{group_id}/providers` (get_group_providers)

**Impact:**
- Any user with `llm_providers::read` permission can retrieve all API keys
- API keys visible in browser developer tools, logs, cache
- Keys can be stolen and used to access external services (OpenAI, Anthropic, etc.)
- Financial impact from unauthorized API usage
- Potential data exfiltration using stolen keys

**Evidence from Code:**

`llm_provider/repository.rs:93-117` - Returns api_key in queries:
```rust
pub async fn get_llm_provider_by_id(
    pool: &PgPool,
    provider_id: Uuid,
) -> Result<Option<LlmProvider>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id, name, provider_type, enabled, api_key, base_url, built_in, proxy_settings, created_at, updated_at
         FROM llm_providers
         WHERE id = $1"#,
        provider_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| LlmProvider {
        id: r.id,
        name: r.name,
        provider_type: r.provider_type,
        enabled: r.enabled,
        api_key: r.api_key,  // ❌ API key included in response
        // ...
    }))
}
```

**Recommended Fix:**

1. Create separate response models with and without secrets:
```rust
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LlmProviderResponse {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
    // api_key omitted
    pub base_url: Option<String>,
    pub built_in: bool,
    pub proxy_settings: ProxySettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LlmProviderWithSecrets {
    // Include api_key only for specific admin endpoints
    pub api_key: Option<String>,
    // ...
}
```

2. Add `#[serde(skip)]` to api_key field in the public model
3. Create admin-only endpoint for retrieving secrets with elevated permissions
4. Audit all existing API responses to ensure no other secrets are exposed

---

## High Severity Findings

### HIGH-1: Repository Credentials Exposed in API Responses

**Severity:** HIGH
**Module:** `llm_repository`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_repository/models.rs`

**Issue:**
Repository authentication credentials (API keys, passwords, tokens) are exposed in all API responses through the `auth_config` field.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct RepositoryAuthConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,  // ❌ HIGH: Exposed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,  // ❌ Potentially sensitive
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,  // ❌ HIGH: Exposed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,  // ❌ HIGH: Exposed
    // ...
}
```

**Impact:**
- Hugging Face API tokens exposed
- Git repository credentials leaked
- Unauthorized access to private repositories
- Potential for data theft from model repositories

**Recommended Fix:**
Same pattern as CRIT-1: separate response models, skip serialization of secrets, admin-only endpoint for credential management.

---

### HIGH-2: SSRF Vulnerability in Repository Downloads

**Severity:** HIGH
**Module:** `llm_model`, `llm_repository`
**Files:**
- `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_model/handlers/uploads.rs:1044-1045`
- `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_repository/handlers.rs:264-298`

**Issue:**
User-controlled URLs are used in outbound HTTP requests without proper validation, allowing Server-Side Request Forgery (SSRF) attacks.

**Vulnerable Code:**

`llm_model/handlers/uploads.rs:1044-1045`:
```rust
let repository_url =
    GitService::build_repository_url(&repository.url, &request.repository_path);
```

`llm_repository/utils.rs:179-262`:
```rust
pub async fn test_repository_connectivity(
    request: &TestRepositoryConnectionRequest,
) -> Result<(), String> {
    // ...
    let test_url = if let Some(auth_config) = &request.auth_config {
        if let Some(ref test_endpoint) = auth_config.auth_test_api_endpoint {
            if !test_endpoint.trim().is_empty() {
                test_endpoint  // ❌ User-controlled URL
            } else {
                &request.url  // ❌ User-controlled URL
            }
        } else {
            &request.url  // ❌ User-controlled URL
        }
    } else {
        &request.url
    };

    // Build the request with authentication
    let mut req_builder = client.get(test_url);  // ❌ SSRF: Makes request to user-controlled URL
```

**Attack Scenarios:**

1. **Internal Network Scanning:**
```json
{
  "name": "SSRF Test",
  "url": "http://localhost:8080/admin",
  "auth_type": "none"
}
```

2. **Cloud Metadata Access (AWS):**
```json
{
  "name": "AWS Metadata",
  "url": "http://169.254.169.254/latest/meta-data/iam/security-credentials/",
  "auth_type": "none"
}
```

3. **Internal Service Exploitation:**
```json
{
  "name": "Redis Exploit",
  "url": "http://internal-redis:6379",
  "auth_type": "none"
}
```

**Impact:**
- Access to internal services (databases, admin panels, cloud metadata)
- Port scanning of internal network
- Potential RCE via Redis/Memcached protocol smuggling
- Bypass of firewall restrictions
- Information disclosure about internal infrastructure

**Recommended Fix:**

1. Implement URL allowlist:
```rust
const ALLOWED_DOMAINS: &[&str] = &[
    "huggingface.co",
    "github.com",
    "gitlab.com",
];

fn validate_repository_url(url: &str) -> Result<(), AppError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| AppError::bad_request("INVALID_URL", "Invalid URL format"))?;

    // Block private IP ranges
    if let Some(host) = parsed.host_str() {
        // Block localhost
        if host == "localhost" || host == "127.0.0.1" || host.starts_with("127.") {
            return Err(AppError::bad_request("BLOCKED_URL", "Localhost access not allowed"));
        }

        // Block private IP ranges
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            if ip.is_loopback() || is_private_ip(&ip) {
                return Err(AppError::bad_request("BLOCKED_URL", "Private IP access not allowed"));
            }
        }

        // Require HTTPS for production
        if parsed.scheme() != "https" {
            return Err(AppError::bad_request("INSECURE_URL", "Only HTTPS URLs allowed"));
        }

        // Domain allowlist (optional but recommended)
        let allowed = ALLOWED_DOMAINS.iter().any(|&domain| {
            host == domain || host.ends_with(&format!(".{}", domain))
        });

        if !allowed {
            return Err(AppError::bad_request("BLOCKED_DOMAIN", "Domain not in allowlist"));
        }
    }

    Ok(())
}

fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            ipv4.is_private() || ipv4.is_link_local() ||
            ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254 // AWS metadata
        }
        std::net::IpAddr::V6(ipv6) => {
            ipv6.is_loopback() ||
            (ipv6.segments()[0] & 0xfe00) == 0xfc00 || // fc00::/7 (ULA)
            (ipv6.segments()[0] & 0xffc0) == 0xfe80    // fe80::/10 (link-local)
        }
    }
}
```

2. Add DNS rebinding protection
3. Set strict timeouts (already at 10s, which is good)
4. Log all outbound requests for security monitoring

---

### HIGH-3: Unrestricted File Upload with Insufficient Validation

**Severity:** HIGH
**Module:** `llm_model`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_model/handlers/uploads.rs:514-865`

**Issue:**
File upload endpoint accepts arbitrary files with minimal validation. While there is some file type checking and HTML detection, the validation is insufficient.

**Vulnerable Code:**

```rust
pub async fn upload_multiple_files_and_commit(
    _auth: RequirePermissions<(LlmModelsCreate,)>,
    mut multipart: Multipart,
) -> ApiResult<Json<LlmModel>> {
    // ...
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        // ...
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "files" => {
                if let Some(file_name) = field.file_name() {
                    let filename = std::path::Path::new(file_name)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(file_name)
                        .to_string();  // ❌ Minimal sanitization

                    let data = field.bytes().await.map_err(|e| {
                        // ...
                    })?;

                    uploaded_files.push((filename, data.to_vec()));
                }
            }
            // ...
        }
    }
}
```

**Validation Issues:**

1. **No file size limit per file** (only total size check later)
2. **Weak MIME type validation:**
```rust
fn validate_file_content(filename: &str, file_data: &[u8]) -> Vec<String> {
    let mut issues = Vec::new();

    if file_data.is_empty() {
        issues.push("File is empty".to_string());
        return issues;
    }

    // Only checks for HTML - misses many malicious file types
    if file_data.len() >= 4 {
        let first_4_bytes = &file_data[0..4];
        if matches!(
            first_4_bytes,
            [0x3C, 0x21, _, _] | [0x3C, 0x68, 0x74, 0x6D] | [0x3C, 0x48, 0x54, 0x4D]
        ) {
            issues.push("File appears to be HTML content".to_string());
        }
    }

    issues  // ❌ Returns warnings but doesn't block upload
}
```

3. **Filename sanitization incomplete:**
```rust
// storage.rs:127-131
let safe_filename = filename
    .replace('/', "_")
    .replace('\\', "_")
    .replace("..", "_");  // ❌ Only handles basic path traversal
```

**Attack Scenarios:**

1. **Path Traversal (partial):**
```
filename: "../../../../etc/passwd"  # Blocked
filename: "..%2F..%2F..%2Fetc%2Fpasswd"  # May bypass
```

2. **Executable Upload:**
```
Upload: malicious.gguf (actually a binary)
Upload: config.json (contains JS/shell code)
```

3. **Resource Exhaustion:**
```
Upload 100 files of 100MB each = 10GB upload (no per-file limits)
```

**Impact:**
- Disk space exhaustion (DoS)
- Potential code execution if files are later executed
- Path traversal with encoded characters
- Storage of malicious files

**Recommended Fix:**

1. Add comprehensive file validation:
```rust
const MAX_FILE_SIZE: usize = 5 * 1024 * 1024 * 1024; // 5GB per file
const MAX_TOTAL_UPLOAD: usize = 50 * 1024 * 1024 * 1024; // 50GB total
const MAX_FILES: usize = 100;

const ALLOWED_EXTENSIONS: &[&str] = &[
    ".gguf", ".bin", ".safetensors", ".pt", ".pth",
    ".json", ".txt", ".model", ".ggml"
];

fn validate_upload_file(filename: &str, data: &[u8]) -> Result<(), AppError> {
    // File size check
    if data.len() > MAX_FILE_SIZE {
        return Err(AppError::bad_request(
            "FILE_TOO_LARGE",
            &format!("File exceeds maximum size of {} bytes", MAX_FILE_SIZE)
        ));
    }

    // Extension check
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s.to_lowercase()));

    if !extension.map(|ext| ALLOWED_EXTENSIONS.contains(&ext.as_str())).unwrap_or(false) {
        return Err(AppError::bad_request(
            "INVALID_FILE_TYPE",
            "File type not allowed"
        ));
    }

    // Magic byte validation
    validate_magic_bytes(filename, data)?;

    // Content scanning
    scan_for_malicious_content(data)?;

    Ok(())
}

fn sanitize_filename(filename: &str) -> Result<String, AppError> {
    // Decode URL encoding
    let decoded = percent_encoding::percent_decode_str(filename)
        .decode_utf8()
        .map_err(|_| AppError::bad_request("INVALID_FILENAME", "Invalid UTF-8"))?;

    // Remove all path components
    let safe_name = std::path::Path::new(decoded.as_ref())
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::bad_request("INVALID_FILENAME", "Invalid filename"))?;

    // Block dangerous characters
    if safe_name.contains(['<', '>', ':', '"', '|', '?', '*', '\0']) {
        return Err(AppError::bad_request("INVALID_FILENAME", "Filename contains invalid characters"));
    }

    // Block hidden files
    if safe_name.starts_with('.') {
        return Err(AppError::bad_request("INVALID_FILENAME", "Hidden files not allowed"));
    }

    Ok(safe_name.to_string())
}
```

2. Add virus scanning integration
3. Implement upload rate limiting
4. Add file count limits

---

### HIGH-4: Information Disclosure in Error Messages

**Severity:** HIGH
**Module:** All modules
**Files:** Multiple handler files

**Issue:**
Detailed internal error messages and stack traces are logged using `eprintln!` and may be exposed to clients. Database errors and internal paths leak implementation details.

**Vulnerable Code Patterns:**

```rust
// llm_provider/handlers.rs:43-44
let all_providers = Repos.llm_provider.list().await.map_err(|e| {
    eprintln!("Failed to get providers: {}", e);  // ❌ Console output (could be logged)
    AppError::internal_error("Database operation failed")  // Generic (good)
})?;

// llm_model/handlers/uploads.rs:295-308
let model_db = repo.create(create_request)
    .await
    .map_err(|e| {
        let error_str = e.to_string();  // ❌ Database error exposed
        tracing::warn!("Database error during model creation: {}", error_str);
        if error_str.contains("llm_models_provider_id_name_unique")
            || (error_str.contains("duplicate key") && error_str.contains("name")) {
            AppError::bad_request(
                "DUPLICATE_ENTRY",
                &format!(
                    "A model with the name '{}' already exists for this provider. Please use a different model name.",
                    model_name  // ❌ User input reflected in error
                )
            )
        } else {
            e  // ❌ Raw database error returned
        }
    })?;
```

**Information Leaked:**
- Database schema details (table names, column names, constraints)
- File system paths
- Internal service names
- SQL error messages
- Stack traces (if debug mode enabled)

**Impact:**
- Aids attackers in reconnaissance
- Exposes database structure for SQL injection attempts
- Reveals internal architecture
- May leak sensitive data in error context

**Recommended Fix:**

1. Create standardized error handling:
```rust
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        // Log detailed error internally
        tracing::error!("Database error: {:?}", err);

        // Return generic error to client
        match err {
            sqlx::Error::Database(db_err) => {
                if db_err.is_unique_violation() {
                    AppError::bad_request("DUPLICATE_ENTRY", "Resource already exists")
                } else if db_err.is_foreign_key_violation() {
                    AppError::bad_request("INVALID_REFERENCE", "Referenced resource not found")
                } else {
                    AppError::internal_error("Database operation failed")
                }
            }
            sqlx::Error::RowNotFound => {
                AppError::not_found("Resource")
            }
            _ => AppError::internal_error("Database operation failed")
        }
    }
}
```

2. Remove `eprintln!` calls - use `tracing` exclusively
3. Never include internal details in client-facing errors
4. Add error tracking/monitoring system

---

## Medium Severity Findings

### MED-1: Missing Authorization Check on Group Assignments

**Severity:** MEDIUM
**Module:** `llm_provider`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/handlers.rs:259-286`

**Issue:**
Users can assign providers to any group without verifying they have permission to manage that specific group.

**Vulnerable Code:**
```rust
pub async fn assign_provider_to_group(
    _auth: RequirePermissions<(LlmProvidersAssignGroups,)>,
    Path(provider_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<AssignProviderToGroupRequest>,
) -> ApiResult<StatusCode> {
    Repos.llm_provider.assign_to_group(provider_id, request.group_id)
        .await
        .map_err(|e| {
            eprintln!(
                "Failed to assign provider {} to group {}: {}",
                provider_id, request.group_id, e
            );
            AppError::internal_error("Database operation failed")
        })?;
    // ❌ No check: Does user have permission to modify request.group_id?
```

**Impact:**
- Users can assign providers to groups they don't manage
- Privilege escalation via group assignment manipulation
- Unauthorized access to providers

**Recommended Fix:**
```rust
// Verify user has permission to modify the target group
let user_groups = get_user_groups(auth.user_id).await?;
if !user_groups.iter().any(|g| g.id == request.group_id && g.can_manage) {
    return Err(AppError::forbidden("Cannot modify this group").into());
}
```

---

### MED-2: No Rate Limiting on Downloads

**Severity:** MEDIUM
**Module:** `llm_model`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_model/handlers/uploads.rs:989-1474`

**Issue:**
No rate limiting on repository downloads allows a single user to exhaust bandwidth and storage.

**Vulnerable Code:**
```rust
pub async fn initiate_repository_download_internal(
    request: DownloadFromRepositoryRequest,
) -> Result<DownloadInstance, String> {
    // ❌ No check for concurrent downloads by this user
    // ❌ No check for total downloads in progress
    // ❌ No bandwidth limiting

    // Spawn background task to handle the download
    tokio::spawn(async move {
        // Large file download with no limits
    });
}
```

**Attack Scenario:**
```bash
# Start 100 concurrent downloads of 50GB models
for i in {1..100}; do
  curl -X POST /api/llm-models/download \
    -d '{"repository_id":"...","provider_id":"..."}' &
done
```

**Impact:**
- Bandwidth exhaustion
- Disk space exhaustion
- Service degradation/DoS
- Cost implications for cloud storage/bandwidth

**Recommended Fix:**
```rust
const MAX_CONCURRENT_DOWNLOADS_PER_USER: usize = 3;
const MAX_TOTAL_DOWNLOADS: usize = 20;

// Check concurrent downloads
let user_downloads = Repos.download_instance
    .get_active_by_user(user_id)
    .await?;

if user_downloads.len() >= MAX_CONCURRENT_DOWNLOADS_PER_USER {
    return Err("Too many concurrent downloads".to_string());
}

// Check total system downloads
let total_downloads = Repos.download_instance
    .get_all_active()
    .await?;

if total_downloads.len() >= MAX_TOTAL_DOWNLOADS {
    return Err("System download limit reached".to_string());
}
```

---

### MED-3: Proxy Credentials in Proxy Settings

**Severity:** MEDIUM
**Module:** `llm_provider`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/models.rs:11-25`

**Issue:**
Proxy credentials (username/password) are stored and returned in the `ProxySettings` struct.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ProxySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,  // ❌ Proxy credential exposed
    #[serde(default)]
    pub password: String,  // ❌ Proxy credential exposed
    #[serde(default)]
    pub no_proxy: String,
    #[serde(default)]
    pub ignore_ssl_certificates: bool,
}
```

**Impact:**
- Proxy credentials leaked via API responses
- Potential for proxy abuse
- Network security compromise

**Recommended Fix:**
Same as CRIT-1 and HIGH-1: separate response models, skip serialization.

---

### MED-4: Console Logging of Sensitive Test URLs

**Severity:** MEDIUM
**Module:** `llm_repository`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_repository/utils.rs:207`

**Issue:**
Repository test URLs (which may contain credentials) are logged to console.

```rust
println!("Testing connection to: {}", test_url);  // ❌ URL may contain credentials
```

**Impact:**
- Credentials exposed in logs
- Log aggregation systems capture secrets
- Compliance violations (PCI-DSS, SOC 2)

**Recommended Fix:**
```rust
// Redact credentials from URLs before logging
fn redact_url(url: &str) -> String {
    if let Ok(mut parsed) = reqwest::Url::parse(url) {
        if parsed.username() != "" || parsed.password().is_some() {
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
        }
        parsed.to_string()
    } else {
        "[INVALID_URL]".to_string()
    }
}

tracing::info!("Testing connection to: {}", redact_url(test_url));
```

---

### MED-5: Insufficient Validation of Model Parameters

**Severity:** MEDIUM
**Module:** `llm_model`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_model/utils.rs:41-44`

**Issue:**
Model parameter validation is delegated to a `validate()` method on the struct, but the validation logic is not visible in the audited code.

```rust
if let Some(ref params) = request.parameters {
    if let Err(e) = params.validate() {  // ❌ Unknown validation logic
        return Err(AppError::unprocessable_entity("INVALID_PARAMETERS", e));
    }
}
```

**Concerns:**
- Temperature, top_p, max_tokens may not have proper bounds
- Negative values might be accepted
- Extremely large values could cause resource exhaustion
- No validation is visible for review

**Recommended Fix:**
Implement explicit validation:
```rust
impl ModelParameters {
    pub fn validate(&self) -> Result<(), &'static str> {
        // Temperature: 0.0 to 2.0
        if let Some(temp) = self.temperature {
            if temp < 0.0 || temp > 2.0 {
                return Err("Temperature must be between 0.0 and 2.0");
            }
        }

        // Top P: 0.0 to 1.0
        if let Some(top_p) = self.top_p {
            if top_p < 0.0 || top_p > 1.0 {
                return Err("Top P must be between 0.0 and 1.0");
            }
        }

        // Max tokens: 1 to 100000
        if let Some(max_tokens) = self.max_tokens {
            if max_tokens < 1 || max_tokens > 100000 {
                return Err("Max tokens must be between 1 and 100000");
            }
        }

        Ok(())
    }
}
```

---

### MED-6: Weak Session ID Generation for Temp Files

**Severity:** MEDIUM
**Module:** `llm_model`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_model/handlers/uploads.rs:790`

**Issue:**
Session IDs for temporary file storage use UUID v4, which is cryptographically random but may be predictable if the RNG is weak.

```rust
let temp_session_id = Uuid::new_v4();  // ❌ Depends on system RNG quality
```

**Impact:**
- Potential session ID guessing
- Unauthorized access to temporary files
- Information disclosure

**Recommended Fix:**
Use cryptographically secure random generation explicitly:
```rust
use rand::Rng;

fn generate_secure_session_id() -> Uuid {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill(&mut bytes);
    Uuid::from_bytes(bytes)
}
```

Note: Uuid::new_v4() likely uses a secure RNG, but this should be verified.

---

## Low Severity Findings

### LOW-1: Missing Input Length Limits

**Severity:** LOW
**Module:** `llm_provider`, `llm_repository`
**Files:** Multiple

**Issue:**
Many string fields lack maximum length validation beyond database constraints.

**Examples:**
- Provider names: validated at 255 chars
- Repository URLs: no visible limit
- Description fields: no visible limit

**Impact:**
- Potential DoS via extremely long inputs
- Database performance degradation
- UI rendering issues

**Recommended Fix:**
Add explicit length checks:
```rust
const MAX_NAME_LENGTH: usize = 255;
const MAX_DESCRIPTION_LENGTH: usize = 5000;
const MAX_URL_LENGTH: usize = 2048;

if name.len() > MAX_NAME_LENGTH {
    return Err(AppError::bad_request("NAME_TOO_LONG", "Name exceeds maximum length"));
}
```

---

### LOW-2: No Audit Logging for Sensitive Operations

**Severity:** LOW
**Module:** All modules
**Files:** N/A

**Issue:**
Sensitive operations (provider creation, API key updates, group assignments) lack comprehensive audit logging.

**Missing Audit Events:**
- API key rotation
- Provider deletion
- Group assignment changes
- Model deletions
- Download initiations

**Impact:**
- Difficulty investigating security incidents
- Compliance violations
- No accountability trail

**Recommended Fix:**
Implement audit logging:
```rust
async fn log_audit_event(
    user_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: Uuid,
    details: serde_json::Value,
) {
    let event = AuditLog {
        user_id,
        action: action.to_string(),
        resource_type: resource_type.to_string(),
        resource_id,
        details,
        timestamp: Utc::now(),
        ip_address: request_ip,
    };

    Repos.audit_log.create(event).await;
}

// Usage:
log_audit_event(
    auth.user_id,
    "provider.api_key.update",
    "llm_provider",
    provider_id,
    json!({"provider_name": provider.name}),
).await;
```

---

### LOW-3: Incomplete Provider File Expiration Handling

**Severity:** LOW
**Module:** `llm_provider_files`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider_files/service.rs:58-68`

**Issue:**
Expired provider files are detected but there's no automatic cleanup mechanism.

```rust
if !is_expired && mapping.upload_status == UploadStatus::Completed {
    if let Some(provider_file_id) = mapping.provider_file_id {
        // Valid mapping exists - return it
        // Note: If provider returns "not found" error later, the caller
        // should handle re-upload (test-and-validate approach)
        return Ok(provider_file_id);
    }
}
```

**Impact:**
- Database bloat with expired mappings
- Stale file references
- Potential confusion when files no longer exist

**Recommended Fix:**
Implement cleanup task:
```rust
async fn cleanup_expired_provider_files(pool: &PgPool) -> Result<usize, AppError> {
    let deleted = sqlx::query!(
        "DELETE FROM llm_provider_files WHERE expires_at < NOW()"
    )
    .execute(pool)
    .await?
    .rows_affected();

    tracing::info!("Cleaned up {} expired provider file mappings", deleted);
    Ok(deleted as usize)
}

// Run periodically (e.g., hourly)
```

---

## Positive Security Practices

The following security practices were observed and should be maintained:

### 1. SQL Injection Prevention
All database queries use SQLx's compile-time checked parameterized queries. **No string formatting in SQL found.**

```rust
// ✅ GOOD: Parameterized query
sqlx::query!(
    "SELECT * FROM llm_providers WHERE id = $1",
    provider_id
)
```

### 2. Permission-Based Access Control
All endpoints require specific permissions using the `RequirePermissions` extractor.

```rust
// ✅ GOOD: Permission requirement
pub async fn list_providers(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    // ...
) -> ApiResult<Json<LlmProviderListResponse>>
```

### 3. Built-in Resource Protection
Built-in providers and repositories cannot be deleted.

```rust
// ✅ GOOD: Built-in protection
if built_in {
    return Ok(Err("Cannot delete built-in provider".to_string()))
}
```

### 4. Path Traversal Prevention (Partial)
Basic path traversal attempts are blocked in file uploads.

```rust
// ✅ GOOD: Basic sanitization
let safe_filename = filename
    .replace('/', "_")
    .replace('\\', "_")
    .replace("..", "_");
```

### 5. Input Validation Patterns
Consistent validation patterns for names, URLs, and auth types.

```rust
// ✅ GOOD: URL validation
pub fn validate_url(url: &str) -> Result<(), AppError> {
    if reqwest::Url::parse(url).is_ok() {
        Ok(())
    } else {
        Err(AppError::bad_request("VALIDATION_ERROR", "Invalid URL format"))
    }
}
```

---

## Remediation Priority

### Immediate Action Required (Sprint 1)

1. **CRIT-1:** Remove API keys from all response models
2. **HIGH-1:** Remove repository credentials from responses
3. **HIGH-2:** Implement SSRF protections with URL allowlist
4. **HIGH-4:** Standardize error handling to prevent information disclosure

### Short Term (Sprint 2-3)

5. **HIGH-3:** Add comprehensive file upload validation
6. **MED-1:** Add group assignment authorization checks
7. **MED-2:** Implement download rate limiting
8. **MED-3:** Remove proxy credentials from responses

### Medium Term (Sprint 4-6)

9. **MED-4:** Replace console logging with secure logging
10. **MED-5:** Add explicit model parameter validation
11. **MED-6:** Verify UUID generation security
12. **LOW-1:** Add comprehensive length limits

### Long Term (Backlog)

13. **LOW-2:** Implement comprehensive audit logging
14. **LOW-3:** Add automated cleanup for expired files

---

## Security Testing Recommendations

### 1. Penetration Testing Focus Areas
- API key extraction via various endpoints
- SSRF exploitation attempts
- File upload bypass techniques
- Path traversal with encoded characters
- Authorization bypass on group assignments

### 2. Automated Security Scanning
- SAST: cargo-audit, clippy with security lints
- DAST: OWASP ZAP against running API
- Dependency scanning: cargo-deny, Dependabot
- Secret scanning: trufflehog, gitleaks

### 3. Manual Security Review
- Review all error handling paths
- Audit all logging statements for sensitive data
- Review all file I/O operations
- Verify all permission checks

---

## Compliance Considerations

### PCI-DSS (if processing payments)
- **3.4:** API keys must be encrypted at rest (not currently implemented)
- **10.2:** Audit logging required for security events (not currently implemented)

### SOC 2 Type II
- **CC6.1:** Logical access controls appear adequate
- **CC6.6:** Audit logs needed for security-relevant events
- **CC7.2:** Secrets in logs violates monitoring requirements

### GDPR (if EU users)
- **Article 32:** Encryption of personal data (API keys) recommended
- **Article 33:** Breach notification requires audit logs

---

## Appendix: Files Reviewed

### llm_provider Module
- models.rs - Provider data models
- types.rs - API request/response types
- repository.rs - Database queries
- handlers.rs - API endpoint handlers
- routes.rs - Route configuration
- utils.rs - Validation utilities
- permissions.rs - Permission definitions

### llm_model Module
- models.rs - Model data structures
- repository.rs - Database operations
- storage.rs - File storage utilities
- handlers/models.rs - Model CRUD handlers
- handlers/uploads.rs - File upload handlers
- handlers/downloads.rs - Download management
- utils.rs - Validation logic

### llm_repository Module
- models.rs - Repository data models
- repository.rs - Database queries
- handlers.rs - API handlers
- utils.rs - Validation and connectivity testing

### llm_provider_files Module
- models.rs - Provider file data models
- repository.rs - Database operations
- service.rs - Business logic for file uploads

---

## Conclusion

The LLM modules contain several critical and high-severity vulnerabilities that require immediate attention. The most pressing issues are:

1. **Credential exposure in API responses** - affects all users immediately
2. **SSRF vulnerabilities** - could lead to internal network compromise
3. **Insufficient file upload validation** - risk of malicious file storage

However, the codebase demonstrates strong fundamentals with parameterized SQL queries and permission-based access control. With focused remediation of the identified issues, the security posture can be significantly improved.

**Recommendation:** Prioritize fixing CRIT-1, HIGH-1, and HIGH-2 before any production deployment.

---

**Report End**
