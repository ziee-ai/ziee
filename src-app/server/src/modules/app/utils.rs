use super::types::SetupAdminRequest;
use crate::common::AppError;
use axum::http::StatusCode;

// =====================================================
// Validation Functions
// =====================================================

pub fn validate_setup_request(req: &SetupAdminRequest) -> Result<(), (StatusCode, AppError)> {
    // Username validation. Closes 13-misc F-06 (Low): reject control
    // chars (incl. RTL override U+202E) and whitespace, which can be
    // used to spoof admin display names in the UI.
    if req.username.len() < 3 || req.username.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_USERNAME", "Username must be 3-100 characters"),
        ));
    }
    if req
        .username
        .chars()
        .any(|c| c.is_control() || c.is_whitespace())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_USERNAME",
                "Username cannot contain whitespace or control characters",
            ),
        ));
    }

    // Display name: same control-char gate when present.
    if let Some(dn) = &req.display_name {
        if dn.len() > 200 {
            return Err((
                StatusCode::BAD_REQUEST,
                AppError::bad_request("INVALID_DISPLAY_NAME", "Display name exceeds 200 chars"),
            ));
        }
        if dn.chars().any(|c| c.is_control()) {
            return Err((
                StatusCode::BAD_REQUEST,
                AppError::bad_request(
                    "INVALID_DISPLAY_NAME",
                    "Display name cannot contain control characters",
                ),
            ));
        }
    }

    // Email validation
    if !is_valid_email(&req.email) {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_EMAIL", "Invalid email format"),
        ));
    }

    // Password strength validation.
    // Uses the shared validate_password_strength helper (auth::password)
    // so the setup-admin flow enforces the same min/max length + NUL-byte
    // checks as registration. Closes 13-misc H-1 (bcrypt 72-byte silent
    // truncation hazard).
    if let Err(msg) = crate::modules::auth::password::validate_password_strength(&req.password)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("WEAK_PASSWORD", msg),
        ));
    }

    Ok(())
}

/// Validate an email address. Closes 13-misc F-05 (Low): the previous
/// hand-rolled check accepted `a@.com`, `a@b..com`, `a@b.c.` and
/// other malformed values. The strictened version rejects:
///   - empty local-part / domain-part / TLD
///   - leading-dot domain (`@.com`)
///   - consecutive dots in domain (`@b..com`)
///   - trailing dot in domain (`@b.c.`)
///   - control chars / whitespace anywhere in the string
///
/// Still hand-rolled (no regex / no email_address crate dep) — RFC
/// 5321 is too permissive for our use case and a full validator
/// would only delay the inevitable verification email when one is
/// wired up (see 01-auth F-12).
pub fn is_valid_email(email: &str) -> bool {
    if email.is_empty() || email.len() > 255 {
        return false;
    }
    if email.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }

    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    // Check local part
    if local.is_empty() || local.len() > 64 {
        return false;
    }

    // Check domain part — must be non-empty, no leading/trailing dot,
    // no consecutive dots, contain at least one dot.
    if domain.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || domain.contains("..")
        || !domain.contains('.')
    {
        return false;
    }

    // Each domain label must be 1..=63 chars and alphanumeric or
    // hyphen (RFC 1035 LDH). The audit's specific failures all
    // bypass this.
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if !label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
    }

    // TLD must be at least 2 alphabetic chars
    let tld = domain.split('.').last().unwrap_or("");
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }

    true
}

pub fn is_strong_password(password: &str) -> bool {
    password.len() >= 8
}
