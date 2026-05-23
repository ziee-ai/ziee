use super::types::SetupAdminRequest;
use crate::common::AppError;
use axum::http::StatusCode;

// =====================================================
// Validation Functions
// =====================================================

pub fn validate_setup_request(req: &SetupAdminRequest) -> Result<(), (StatusCode, AppError)> {
    // Username validation
    if req.username.len() < 3 || req.username.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_USERNAME", "Username must be 3-100 characters"),
        ));
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

pub fn is_valid_email(email: &str) -> bool {
    // Basic email validation without regex
    if email.is_empty() || email.len() > 255 {
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

    // Check domain part
    if domain.is_empty() || !domain.contains('.') {
        return false;
    }

    // Check domain has valid TLD
    let domain_parts: Vec<&str> = domain.split('.').collect();
    if domain_parts.len() < 2 {
        return false;
    }

    let tld = domain_parts.last().unwrap();
    if tld.len() < 2 {
        return false;
    }

    true
}

pub fn is_strong_password(password: &str) -> bool {
    password.len() >= 8
}
