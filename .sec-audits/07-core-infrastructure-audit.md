# Core Infrastructure Security Audit

**Date:** 2025-11-21
**Scope:** Core infrastructure in `/home/pbya/projects/ziee-chat/src-app/server/src/`
**Focus Areas:** common/, core/, main.rs, lib.rs, build.rs, configuration handling

---

## Executive Summary

This audit identified **10 security issues** across the core infrastructure, ranging from CRITICAL to LOW severity. The most significant findings include:

- **CRITICAL:** Disabled request body size limits creating DoS vulnerability
- **CRITICAL:** Hardcoded database password in build.rs printed to console
- **HIGH:** Weak development JWT secret with insufficient entropy
- **HIGH:** Overly permissive default CORS configuration
- **HIGH:** Missing rate limiting on all endpoints
- **MEDIUM:** Database connection strings logged with credentials
- **MEDIUM:** Detailed error messages expose internal implementation

---

## Critical Findings

### 1. Disabled Request Body Size Limits (DoS Vulnerability)

**Severity:** CRITICAL
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/main.rs:172`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/lib.rs:111`

**Issue:**
```rust
// main.rs line 172
let app = api_router
    .finish_api(&mut api_doc)
    .layer(axum::extract::DefaultBodyLimit::disable())  // ← CRITICAL: No size limit!
    .layer(axum::Extension(event_bus))
```

**Vulnerability:**
- Request body size limits are completely disabled across ALL endpoints
- Comment says "Disable body size limit for model uploads (models can be very large)"
- This creates a severe Denial of Service (DoS) vulnerability
- Attackers can send arbitrarily large requests to exhaust server memory/disk space
- Applies to ALL routes, not just model uploads

**Impact:**
- **Memory exhaustion:** Large requests can consume all available RAM
- **Disk space exhaustion:** Request buffering can fill disk
- **Network saturation:** Sustained large uploads can saturate bandwidth
- **Application crash:** OOM killer may terminate the process
- **Service unavailability:** Legitimate users cannot access the service

**Recommended Fix:**
```rust
// Set a large but reasonable default limit (e.g., 100MB)
let default_body_limit = 100 * 1024 * 1024; // 100MB

let app = api_router
    .finish_api(&mut api_doc)
    .layer(axum::extract::DefaultBodyLimit::max(default_body_limit))
    .layer(axum::Extension(event_bus))
    .layer(axum::Extension(jwt_service))
    .layer(cors);

// Then, for specific routes that need larger limits (like model uploads),
// override with route-specific configuration:
// .route("/api/llm-models/upload",
//    post(upload_handler)
//      .layer(axum::extract::DefaultBodyLimit::max(5 * 1024 * 1024 * 1024)) // 5GB
// )
```

**References:**
- CWE-400: Uncontrolled Resource Consumption
- OWASP: Denial of Service
- Axum DefaultBodyLimit documentation

---

### 2. Hardcoded Database Password Exposed in Build Process

**Severity:** CRITICAL
**File:** `/home/pbya/projects/ziee-chat/src-app/server/build.rs:12-26`

**Issue:**
```rust
// build.rs line 12-13
let database_url = env::var("DATABASE_URL")
    .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:54321/postgres".to_string());

// Line 26
eprintln!("DATABASE_URL: {}", database_url);
```

**Vulnerability:**
- Hardcoded database password "password" in default connection string
- Password is printed to stderr during every build (line 26)
- Build logs are often stored/archived with credentials visible
- Default credentials may persist in production builds
- Password visible in build output on CI/CD systems

**Impact:**
- **Credential exposure:** Database password visible in build logs
- **Production risk:** Default password may be used in production
- **Log retention:** Credentials stored in CI/CD logs indefinitely
- **Supply chain attack:** Compromised build systems expose credentials

**Recommended Fix:**
```rust
// build.rs
let database_url = env::var("DATABASE_URL")
    .unwrap_or_else(|_| {
        // Use a more secure default or require explicit configuration
        "postgresql://postgres@127.0.0.1:54321/postgres".to_string()
    });

// NEVER log connection strings with credentials
if let Err(e) = pool_result {
    eprintln!("\nERROR: Failed to connect to database: {}", e);
    // DO NOT print DATABASE_URL
    eprintln!("Ensure DATABASE_URL environment variable is set correctly");
    panic!("Database connection failed");
}
```

**Additional Recommendations:**
- Remove password from default connection string
- Require explicit DATABASE_URL environment variable for builds
- Never log connection strings containing credentials
- Use connection string parsing to redact passwords before logging

**References:**
- CWE-798: Use of Hard-coded Credentials
- CWE-532: Insertion of Sensitive Information into Log File
- OWASP: Sensitive Data Exposure

---

## High Severity Findings

### 3. Weak JWT Secret in Development Configuration

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/config/dev.yaml:81`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/config/dev.example.yaml:81`

**Issue:**
```yaml
# JWT authentication settings
jwt:
  secret: "dev-secret-change-in-production-min-32-chars-long"
  issuer: "ziee-chat"
  audience: "ziee-chat-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: 30
```

**Vulnerability:**
- Development JWT secret is a simple, human-readable string
- Low entropy (~50 bits) vs. recommended 256+ bits
- Same secret present in both dev.yaml and dev.example.yaml
- No validation that secret is changed in production
- Secret easily guessable/brute-forceable

**Impact:**
- **Token forgery:** Attackers can forge valid JWT tokens
- **Authentication bypass:** Complete authentication bypass possible
- **Session hijacking:** Can impersonate any user including admins
- **Privilege escalation:** Can create admin tokens from user tokens

**Recommended Fix:**

1. **Generate cryptographically secure secrets:**
```bash
# Generate a proper JWT secret (256 bits of entropy)
openssl rand -base64 32
# Example output: 7xK9mP2nQ5wR8tY4uZ6vA3bC1dE0fG2hJ4kL6mN8oP0qS=
```

2. **Add runtime validation in config.rs:**
```rust
impl Config {
    pub fn load_from(config_path: Option<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // ... existing code ...

        // Validate JWT secret strength
        if config.jwt.secret.len() < 32 {
            return Err("JWT secret must be at least 32 characters long".into());
        }

        // Warn if using example/weak secrets
        let weak_secrets = vec![
            "dev-secret-change-in-production-min-32-chars-long",
            "change-me",
            "secret",
        ];
        if weak_secrets.contains(&config.jwt.secret.as_str()) {
            return Err("SECURITY WARNING: Cannot use example/weak JWT secret in production. Generate a secure secret with: openssl rand -base64 32".into());
        }

        Ok(config)
    }
}
```

3. **Update example config:**
```yaml
jwt:
  # SECURITY: Generate a secure secret with: openssl rand -base64 32
  # NEVER commit production secrets to version control
  secret: "${JWT_SECRET:?Environment variable JWT_SECRET is required}"
  issuer: "ziee-chat"
  audience: "ziee-chat-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: 30
```

**References:**
- CWE-798: Use of Hard-coded Credentials
- CWE-326: Inadequate Encryption Strength
- RFC 7519: JSON Web Token (JWT)
- OWASP: Cryptographic Failures

---

### 4. Overly Permissive CORS Configuration

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/core/app_builder.rs:100-157`

**Issue:**
```rust
pub fn create_cors_layer(config: &Config) -> CorsLayer {
    if let Some(ref cors_config) = config.server.cors {
        // ... parsing code ...

        // Set origins
        if cors_config.allow_origins.contains(&"*".to_string()) || origins.is_empty() {
            layer = layer.allow_origin(Any);  // ← Accepts ALL origins
        }

        // Set methods
        if methods.is_empty() {
            layer = layer.allow_methods(Any);  // ← Allows ALL HTTP methods
        }

        // Set headers
        if cors_config.allow_headers.contains(&"*".to_string()) || headers.is_empty() {
            layer = layer.allow_headers(Any);  // ← Allows ALL headers
        }
    } else {
        // Default permissive CORS if not configured
        CorsLayer::new()
            .allow_origin(Any)      // ← CRITICAL: Default accepts ANY origin
            .allow_methods(Any)     // ← CRITICAL: Default allows ANY method
            .allow_headers(Any)     // ← CRITICAL: Default allows ANY header
    }
}
```

**Vulnerability:**
- If CORS configuration is missing, defaults to allowing ALL origins
- Wildcard "*" in allow_origins enables all origins
- Empty arrays default to permissive "Any" setting
- No distinction between development and production environments

**Impact:**
- **CSRF attacks:** Any website can make authenticated requests
- **Data exfiltration:** Malicious sites can read user data
- **Session hijacking:** Credentials can be sent to attacker origins
- **API abuse:** Unauthorized websites can consume API resources

**Recommended Fix:**
```rust
pub fn create_cors_layer(config: &Config) -> CorsLayer {
    let cors_config = config.server.cors.as_ref()
        .ok_or("CORS configuration is required for security")
        .expect("CORS not configured - refusing to start with insecure defaults");

    // NEVER allow wildcard origins in production
    let origins: Vec<_> = cors_config
        .allow_origins
        .iter()
        .filter_map(|origin| {
            if origin == "*" {
                tracing::warn!("SECURITY WARNING: Wildcard CORS origin '*' should only be used in development");
                // In production, reject wildcard
                if is_production() {
                    panic!("Wildcard CORS origins are not allowed in production");
                }
                None
            } else {
                origin.parse::<axum::http::HeaderValue>().ok()
            }
        })
        .collect();

    if origins.is_empty() {
        panic!("No valid CORS origins configured - refusing to start");
    }

    let mut layer = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins));

    // Explicit method allowlist (never use Any)
    let methods: Vec<Method> = cors_config
        .allow_methods
        .iter()
        .filter_map(|m| m.parse().ok())
        .collect();

    if methods.is_empty() {
        // Safe default: only GET, POST, PUT, DELETE
        layer = layer.allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ]);
    } else {
        layer = layer.allow_methods(methods);
    }

    // Explicit header allowlist
    let headers: Vec<HeaderName> = cors_config
        .allow_headers
        .iter()
        .filter_map(|h| {
            if h == "*" {
                tracing::warn!("Wildcard CORS header should be avoided");
                None
            } else {
                h.parse().ok()
            }
        })
        .collect();

    if !headers.is_empty() {
        layer = layer.allow_headers(headers);
    } else {
        // Safe default headers
        layer = layer.allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);
    }

    layer
}
```

**Configuration Recommendations:**
```yaml
# Development
server:
  cors:
    allow_origins:
      - "http://localhost:5173"
      - "http://localhost:3000"
    allow_methods:
      - "GET"
      - "POST"
      - "PUT"
      - "DELETE"
      - "OPTIONS"
    allow_headers:
      - "Content-Type"
      - "Authorization"

# Production - be even more restrictive
server:
  cors:
    allow_origins:
      - "https://yourdomain.com"
      - "https://www.yourdomain.com"
    allow_methods:
      - "GET"
      - "POST"
      - "PUT"
      - "DELETE"
    allow_headers:
      - "Content-Type"
      - "Authorization"
```

**References:**
- CWE-942: Permissive Cross-domain Policy with Untrusted Domains
- OWASP: CORS Misconfiguration
- MDN: Cross-Origin Resource Sharing (CORS)

---

### 5. Missing Rate Limiting

**Severity:** HIGH
**Files:** All core infrastructure files

**Issue:**
- No rate limiting middleware is implemented or configured
- All endpoints are vulnerable to brute-force attacks
- No protection against API abuse or DoS
- Authentication endpoints lack attempt throttling

**Vulnerability:**
- Authentication endpoints can be brute-forced without limits
- API endpoints can be spammed to exhaust resources
- No per-user or per-IP rate limits
- No differentiation between authenticated/unauthenticated users

**Impact:**
- **Brute-force attacks:** Unlimited password guessing attempts
- **API abuse:** Resource exhaustion through excessive requests
- **DoS attacks:** Service degradation from high request volume
- **Cost escalation:** Excessive database/compute usage

**Recommended Fix:**

1. **Add tower-governor or similar rate limiting:**
```rust
// Add to Cargo.toml
tower-governor = "0.3"

// In app_builder.rs or main.rs
use tower_governor::{
    governor::GovernorConfigBuilder,
    GovernorLayer,
};
use std::time::Duration;

// Create rate limiter configuration
let governor_conf = Box::new(
    GovernorConfigBuilder::default()
        .per_second(10)  // 10 requests per second
        .burst_size(30)  // Allow bursts up to 30 requests
        .finish()
        .unwrap()
);

let app = api_router
    .finish_api(&mut api_doc)
    .layer(GovernorLayer {
        config: Box::leak(governor_conf),
    })
    .layer(axum::extract::DefaultBodyLimit::max(100_000_000))
    .layer(axum::Extension(event_bus))
    .layer(axum::Extension(jwt_service))
    .layer(cors);
```

2. **Implement stricter limits for auth endpoints:**
```rust
// More aggressive rate limiting for authentication
let auth_limiter = Box::new(
    GovernorConfigBuilder::default()
        .per_minute(5)   // Only 5 login attempts per minute
        .burst_size(10)  // Burst of 10 max
        .finish()
        .unwrap()
);

// Apply to auth routes specifically
let auth_routes = Router::new()
    .route("/login", post(login_handler))
    .route("/refresh", post(refresh_handler))
    .layer(GovernorLayer {
        config: Box::leak(auth_limiter),
    });
```

3. **Add configuration for rate limits:**
```yaml
# config/dev.yaml
server:
  rate_limiting:
    enabled: true
    requests_per_second: 10
    burst_size: 30
    auth_requests_per_minute: 5
    auth_burst_size: 10
```

**References:**
- CWE-307: Improper Restriction of Excessive Authentication Attempts
- CWE-770: Allocation of Resources Without Limits or Throttling
- OWASP: API4:2023 Unrestricted Resource Consumption
- OWASP: Brute Force

---

## Medium Severity Findings

### 6. Database Connection String Logging

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/core/database/mod.rs:205`

**Issue:**
```rust
// database/mod.rs line 205
let database_url = postgresql.settings().url("postgres");
println!("Generated database_url: {:?}", database_url);
```

**Vulnerability:**
- Database connection URLs are logged to stdout
- Connection strings contain username and password
- Logs may be stored, transmitted, or viewed by unauthorized users
- Similar logging in build.rs (already covered in finding #2)

**Impact:**
- **Credential disclosure:** Database credentials visible in logs
- **Log retention risk:** Credentials stored in log files indefinitely
- **Monitoring exposure:** Credentials visible to log aggregation systems

**Recommended Fix:**
```rust
// Create a helper function to redact credentials
fn redact_connection_string(url: &str) -> String {
    // postgresql://user:password@host:port/db
    // becomes postgresql://user:***@host:port/db
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let mut redacted = url.to_string();
            redacted.replace_range((colon_pos + 1)..at_pos, "***");
            return redacted;
        }
    }
    url.to_string()
}

// Use redacted logging
let database_url = postgresql.settings().url("postgres");
println!("Connected to database: {}", redact_connection_string(&database_url));
```

**References:**
- CWE-532: Insertion of Sensitive Information into Log File
- OWASP: Sensitive Data Exposure

---

### 7. Detailed Error Messages in Production

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/common/type.rs:109-115`

**Issue:**
```rust
pub fn database_error(err: impl std::error::Error) -> Self {
    Self::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "SYSTEM_DATABASE_ERROR",
        format!("Database error: {}", err),  // ← Exposes internal error details
    )
}
```

**Vulnerability:**
- Database error messages are returned to clients verbatim
- Error messages may reveal:
  - Database schema information (table/column names)
  - SQL query structure
  - Internal implementation details
  - File paths and system information
- No differentiation between development and production

**Impact:**
- **Information disclosure:** Attackers learn about internal structure
- **Attack surface mapping:** Error messages reveal attack vectors
- **Schema enumeration:** Table and column names exposed
- **SQL injection testing:** Error messages aid injection attempts

**Recommended Fix:**
```rust
pub fn database_error(err: impl std::error::Error) -> Self {
    // Log the full error for debugging
    tracing::error!("Database error occurred: {}", err);

    // Return generic message to client
    Self::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "SYSTEM_DATABASE_ERROR",
        "An internal database error occurred. Please try again later.",
    )
}

// For development, add environment-based verbose errors
pub fn database_error_verbose(err: impl std::error::Error) -> Self {
    let message = if cfg!(debug_assertions) {
        // Development: show detailed error
        format!("Database error: {}", err)
    } else {
        // Production: generic message
        "An internal database error occurred. Please try again later.".to_string()
    };

    // Always log the full error
    tracing::error!("Database error: {}", err);

    Self::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "SYSTEM_DATABASE_ERROR",
        message,
    )
}
```

**References:**
- CWE-209: Generation of Error Message Containing Sensitive Information
- OWASP: Improper Error Handling
- OWASP: Information Exposure Through Error Messages

---

### 8. No Input Validation in Config Loading

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/core/config.rs:109-154`

**Issue:**
```rust
pub fn load_from(config_path: Option<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
    // ... loads config ...

    // Minimal validation
    if config.postgresql.use_embedded && config.postgresql.embedded.is_none() {
        return Err("use_embedded is true but embedded configuration is missing".into());
    }

    // No validation of:
    // - Port ranges (could be 0-65535)
    // - Host addresses (could be invalid)
    // - JWT secret strength
    // - CORS origin format
    // - Pool connection limits
    // - Token expiry sanity
}
```

**Vulnerability:**
- Configuration values are not validated for security or correctness
- Invalid values may cause runtime failures or security issues
- No bounds checking on numeric values
- No format validation on URLs/addresses

**Impact:**
- **Configuration errors:** Invalid config causes runtime failures
- **Security misconfigurations:** Weak settings accepted without warning
- **Service disruption:** Bad values cause startup/runtime failures

**Recommended Fix:**
```rust
impl Config {
    pub fn load_from(config_path: Option<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // ... existing loading code ...

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Validate JWT configuration
        if self.jwt.secret.len() < 32 {
            return Err("JWT secret must be at least 32 characters".into());
        }

        if self.jwt.access_token_expiry_hours < 1 || self.jwt.access_token_expiry_hours > 720 {
            return Err("Access token expiry must be between 1 and 720 hours".into());
        }

        if self.jwt.refresh_token_expiry_days < 1 || self.jwt.refresh_token_expiry_days > 365 {
            return Err("Refresh token expiry must be between 1 and 365 days".into());
        }

        // Validate pool configuration
        if let Some(ref pool) = self.postgresql.pool {
            if pool.max_connections < pool.min_connections {
                return Err("max_connections must be >= min_connections".into());
            }

            if pool.max_connections > 1000 {
                return Err("max_connections should not exceed 1000".into());
            }

            if pool.acquire_timeout_secs > 300 {
                return Err("acquire_timeout_secs should not exceed 300 seconds".into());
            }
        }

        // Validate CORS origins format
        if let Some(ref cors) = self.server.cors {
            for origin in &cors.allow_origins {
                if origin != "*" && !origin.starts_with("http://") && !origin.starts_with("https://") {
                    return Err(format!("Invalid CORS origin format: {}", origin).into());
                }
            }
        }

        // Validate server configuration
        if self.server.port > 0 && self.server.port < 1024 {
            tracing::warn!("Using privileged port {} - requires elevated permissions", self.server.port);
        }

        Ok(())
    }
}
```

**References:**
- CWE-20: Improper Input Validation
- OWASP: Server-Side Request Forgery (SSRF)

---

### 9. Unvalidated Binary Downloads in Build Process

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/build_helper/pandoc.rs:10-18`
**File:** `/home/pbya/projects/ziee-chat/src-app/server/build_helper/pdfium.rs:4-16`

**Issue:**
```rust
// pandoc.rs and pdfium.rs
fn download_binary(
    url: &str,
    target_path: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading {} from: {}", name, url);

    let response = ureq::get(url).call()?;
    let bytes = response.into_body().read_to_vec()?;

    fs::write(target_path, &bytes)?;  // ← No integrity verification!

    Ok(())
}
```

**Vulnerability:**
- Downloaded binaries (Pandoc, PDFium) have no checksum/signature verification
- Man-in-the-middle attacks could inject malicious binaries
- Compromised GitHub releases could distribute malware
- Downloaded binaries are embedded directly into the compiled application
- No HTTPS certificate pinning

**Impact:**
- **Supply chain attack:** Malicious binaries embedded in application
- **Code execution:** Compromised binaries execute with app privileges
- **Data exfiltration:** Malware in embedded binaries can steal data
- **Backdoor installation:** Attackers gain persistent access

**Recommended Fix:**
```rust
use sha2::{Sha256, Digest};

// Define expected checksums for each binary version
const PANDOC_CHECKSUMS: &[(&str, &str)] = &[
    ("3.7.0.2", "linux-amd64", "abc123..."),  // SHA-256 checksum
    ("3.7.0.2", "windows-x86_64", "def456..."),
    // ... etc
];

fn download_binary_verified(
    url: &str,
    target_path: &Path,
    name: &str,
    expected_checksum: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading {} from: {}", name, url);

    let response = ureq::get(url).call()?;
    let bytes = response.into_body().read_to_vec()?;

    // Verify checksum
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let checksum = format!("{:x}", hasher.finalize());

    if checksum != expected_checksum {
        return Err(format!(
            "Checksum verification failed for {}!\nExpected: {}\nGot: {}",
            name, expected_checksum, checksum
        ).into());
    }

    println!("Checksum verified for {}", name);
    fs::write(target_path, &bytes)?;

    Ok(())
}
```

**Additional Recommendations:**
1. Pin to specific versions (already done with PANDOC_VERSION)
2. Verify checksums against published SHA-256 hashes
3. Consider downloading from multiple mirrors and comparing
4. Add subresource integrity checks
5. Document the expected checksums in version control

**References:**
- CWE-494: Download of Code Without Integrity Check
- CWE-829: Inclusion of Functionality from Untrusted Control Sphere
- OWASP: Software and Data Integrity Failures

---

### 10. Global State Without Proper Synchronization Checks

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/core/app_state.rs:8-39`

**Issue:**
```rust
pub static APP_DATA_DIR: Lazy<Mutex<PathBuf>> = Lazy::new(|| {
    let default_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ziee");
    Mutex::new(default_path)
});

pub fn set_app_data_dir(path: PathBuf) {
    if let Ok(mut app_data_dir) = APP_DATA_DIR.lock() {
        *app_data_dir = path;
        tracing::info!("Application data directory set to: {}", app_data_dir.display());
    } else {
        tracing::error!("Failed to lock APP_DATA_DIR mutex");  // ← Silent failure!
    }
}

pub fn get_app_data_dir() -> PathBuf {
    APP_DATA_DIR
        .lock()
        .expect("Failed to lock APP_DATA_DIR")  // ← Panic on poison
        .clone()
}
```

**Vulnerability:**
- `set_app_data_dir()` silently fails if mutex is poisoned
- `get_app_data_dir()` panics on mutex poison instead of handling gracefully
- No protection against concurrent modification
- Mutex poisoning can cause application-wide failures

**Impact:**
- **Silent failures:** Configuration changes may not apply
- **Application crashes:** Panic in get_app_data_dir() crashes threads
- **Inconsistent state:** Race conditions during initialization
- **Service disruption:** Poisoned mutex makes data dir inaccessible

**Recommended Fix:**
```rust
use std::sync::RwLock;  // Use RwLock for read-heavy access
use once_cell::sync::Lazy;

pub static APP_DATA_DIR: Lazy<RwLock<PathBuf>> = Lazy::new(|| {
    let default_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ziee-chat");
    RwLock::new(default_path)
});

pub fn set_app_data_dir(path: PathBuf) -> Result<(), String> {
    match APP_DATA_DIR.write() {
        Ok(mut app_data_dir) => {
            *app_data_dir = path;
            tracing::info!("Application data directory set to: {}", app_data_dir.display());
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to acquire write lock on APP_DATA_DIR: {}", e);
            tracing::error!("{}", err_msg);
            Err(err_msg)
        }
    }
}

pub fn get_app_data_dir() -> Result<PathBuf, String> {
    match APP_DATA_DIR.read() {
        Ok(app_data_dir) => Ok(app_data_dir.clone()),
        Err(e) => {
            let err_msg = format!("Failed to acquire read lock on APP_DATA_DIR: {}", e);
            tracing::error!("{}", err_msg);
            Err(err_msg)
        }
    }
}

// Or use OnceLock for write-once semantics if dir shouldn't change after init
use std::sync::OnceLock;

pub static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_app_data_dir(path: PathBuf) -> Result<(), String> {
    APP_DATA_DIR.set(path.clone())
        .map_err(|_| "APP_DATA_DIR already initialized".to_string())?;
    tracing::info!("Application data directory set to: {}", path.display());
    Ok(())
}

pub fn get_app_data_dir() -> &'static PathBuf {
    APP_DATA_DIR.get()
        .expect("APP_DATA_DIR not initialized - call set_app_data_dir first")
}
```

**References:**
- CWE-362: Concurrent Execution using Shared Resource with Improper Synchronization
- CWE-609: Double-Checked Locking
- Rust Book: Fearless Concurrency

---

## Low Severity Findings

### 11. Panic on Configuration Errors

**Severity:** LOW
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/main.rs:60-64`

**Issue:**
```rust
let config = match core::config::Config::load_from(cli.config_file) {
    Ok(cfg) => cfg,
    Err(e) => {
        eprintln!("Failed to load configuration: {}", e);
        std::process::exit(1);  // ← Abrupt termination
    }
};
```

**Vulnerability:**
- Application exits immediately on configuration errors
- No opportunity for graceful degradation
- Supervisord/systemd may restart in a loop
- No telemetry/alerting on config failures

**Impact:**
- **Service unavailability:** Config errors prevent startup
- **Restart loops:** Automated systems may repeatedly restart
- **No fallback:** Cannot run with default/fallback configuration

**Recommended Fix:**
```rust
// Allow fallback to defaults with warnings for non-critical settings
let config = match core::config::Config::load_from(cli.config_file) {
    Ok(cfg) => cfg,
    Err(e) => {
        eprintln!("ERROR: Failed to load configuration: {}", e);
        eprintln!("Please check your configuration file and try again.");
        eprintln!("Example configuration: config/dev.example.yaml");

        // Emit metric/alert if using telemetry
        // metrics::increment_counter!("config_load_failures");

        std::process::exit(1);
    }
};
```

**References:**
- CWE-705: Incorrect Control Flow Scoping
- Twelve-Factor App: Config

---

### 12. Missing Security Headers

**Severity:** LOW
**Files:** All routing/middleware files

**Issue:**
- No security headers are configured:
  - No `X-Content-Type-Options: nosniff`
  - No `X-Frame-Options: DENY`
  - No `X-XSS-Protection: 1; mode=block`
  - No `Strict-Transport-Security` (HSTS)
  - No `Content-Security-Policy` (CSP)
  - No `Referrer-Policy`

**Impact:**
- **Clickjacking:** Application can be embedded in iframes
- **MIME sniffing:** Browser may misinterpret content types
- **Missing HSTS:** HTTPS not enforced, vulnerable to downgrade attacks

**Recommended Fix:**
```rust
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::header::{HeaderName, HeaderValue};

// Add security headers middleware
let app = api_router
    .finish_api(&mut api_doc)
    .layer(SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    ))
    // ... other layers ...
```

**References:**
- OWASP: Security Headers
- Mozilla Observatory

---

## Positive Security Practices

The audit also identified several **good security practices** already in place:

### 1. Strong Password Hashing
- **File:** `src/modules/auth/password.rs`
- Uses bcrypt with appropriate cost factor (DEFAULT_COST = 12)
- Automatic salt generation
- Constant-time comparison via `verify()`

### 2. Proper JWT Validation
- **File:** `src/modules/auth/jwt.rs`
- Validates issuer, audience, and expiration
- Separate access/refresh token validation
- Uses industry-standard jsonwebtoken crate

### 3. Prepared Statements (SQLx)
- All database queries use SQLx compile-time checked queries
- No string concatenation for SQL
- Protection against SQL injection

### 4. Type-Safe Configuration
- Strong typing for configuration values
- Validation of required fields
- No eval/exec of configuration values

### 5. Database Connection Pooling
- Proper connection pool management with limits
- Configurable timeouts and lifetimes
- Connection leak prevention

---

## Summary of Recommendations

### Immediate Actions (Critical/High)

1. **Body Size Limits:** Implement default 100MB limit, per-route overrides for uploads
2. **Build Credentials:** Remove hardcoded password, never log connection strings
3. **JWT Secret:** Require strong secrets, validate at runtime, fail if weak
4. **CORS:** Require explicit configuration, reject wildcard origins in production
5. **Rate Limiting:** Implement global and auth-specific rate limits

### Short-term Actions (Medium)

6. **Error Handling:** Generic errors in production, detailed logging server-side
7. **Config Validation:** Validate all configuration values at load time
8. **Binary Verification:** Add checksum verification for downloaded binaries
9. **Global State:** Use OnceLock or RwLock with proper error handling

### Long-term Improvements (Low)

10. **Security Headers:** Add comprehensive security header middleware
11. **Configuration Fallback:** Graceful degradation for non-critical config errors
12. **Monitoring:** Add telemetry for security events and configuration failures

---

## Testing Recommendations

1. **Penetration Testing:**
   - Test body size limits with large payloads
   - Attempt CORS bypass with various origins
   - Brute-force authentication endpoints
   - Test rate limiting effectiveness

2. **Configuration Testing:**
   - Test with missing/invalid configuration
   - Verify JWT secret validation
   - Test CORS with wildcard/empty configs
   - Verify error message sanitization

3. **Integration Testing:**
   - Test global state under concurrent access
   - Verify database connection string redaction
   - Test graceful shutdown with active connections
   - Verify embedded binary integrity

4. **Security Scanning:**
   - Run dependency audit: `cargo audit`
   - Static analysis: `cargo clippy -- -W clippy::all`
   - Check for vulnerable dependencies
   - Scan binaries for backdoors

---

## Compliance Considerations

### OWASP Top 10 2021

- **A01 Broken Access Control:** Addressed by CORS and auth improvements
- **A02 Cryptographic Failures:** JWT secret strength, TLS enforcement
- **A03 Injection:** SQLx prepared statements (already secure)
- **A04 Insecure Design:** Rate limiting, input validation
- **A05 Security Misconfiguration:** Config validation, security headers
- **A07 Identification and Authentication Failures:** Rate limiting, JWT validation
- **A09 Security Logging and Monitoring Failures:** Redact sensitive data from logs

### CWE Top 25

- CWE-20: Improper Input Validation (Config validation)
- CWE-78: OS Command Injection (N/A - no shell commands)
- CWE-79: Cross-site Scripting (Frontend concern)
- CWE-89: SQL Injection (Protected by SQLx)
- CWE-200: Exposure of Sensitive Information (Error messages, logging)
- CWE-287: Improper Authentication (JWT secret strength)
- CWE-306: Missing Authentication (Auth middleware)
- CWE-352: Cross-Site Request Forgery (CORS configuration)
- CWE-400: Uncontrolled Resource Consumption (Body limits, rate limiting)
- CWE-798: Use of Hard-coded Credentials (Build.rs password)

---

## Conclusion

The Ziee Chat core infrastructure has a **solid foundation** with proper use of:
- SQLx for SQL injection prevention
- Bcrypt for password hashing
- JWT for authentication
- Type-safe configuration

However, the **CRITICAL findings** require immediate attention:
1. Disabled body size limits expose the application to DoS attacks
2. Hardcoded database credentials in build process
3. Weak JWT secret and overly permissive CORS in development configs could leak to production
4. Missing rate limiting allows brute-force and abuse

Addressing the critical and high-severity findings will significantly improve the security posture of the application.

---

**Auditor Notes:**
- This audit focused on core infrastructure only
- Module-specific security (auth, user, file, etc.) should be audited separately
- Frontend security (XSS, CSRF tokens) is out of scope
- Network security (TLS configuration, firewall rules) is out of scope
- Review recommended fixes before implementation
- Test all changes in a development environment first
