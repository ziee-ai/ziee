use super::types::SetupAdminRequest;
use crate::common::AppError;
// Shared username + display-name rules — ONE definition for first-run setup and
// every runtime write path (register / profile / admin-create / admin-edit).
use crate::modules::auth::username::{validate_display_name, validate_username};
use axum::http::StatusCode;

// =====================================================
// Validation Functions
// =====================================================

pub fn validate_setup_request(req: &SetupAdminRequest) -> Result<(), (StatusCode, AppError)> {
    // Username validation, delegated to the shared `auth::username` gate so
    // first-run setup and the four runtime username write paths (register /
    // profile / admin-create / admin-edit) cannot drift apart. Keeps the
    // long-standing 3-100 bound + control/whitespace/bidi rejection (13-misc
    // F-06) and adds the charset allowlist.
    validate_username(&req.username).map_err(AppError::to_api_error)?;

    // Display name, delegated to the shared `auth::username` gate for the same
    // no-drift reason as the username above. The bound moves from this path's
    // hand-rolled 200 BYTES to the column's real 255 CHARACTERS — a legitimate
    // 100-character CJK display name was being rejected here while Postgres
    // would have stored it fine.
    if let Some(dn) = &req.display_name {
        validate_display_name(dn).map_err(AppError::to_api_error)?;
    }

    // Email validation
    validate_email(&req.email).map_err(AppError::to_api_error)?;

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

/// `is_valid_email` as a `Result`, so every write path can gate on it with `?`
/// and get the same `400 INVALID_EMAIL` body. Added when `POST /api/users`
/// turned out to gate email on `is_empty()` alone — an over-long or
/// NUL-bearing address reached Postgres as a raw 500.
pub fn validate_email(email: &str) -> Result<(), AppError> {
    if is_valid_email(email) {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "INVALID_EMAIL",
            "Invalid email format",
        ))
    }
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
    let tld = domain.split('.').next_back().unwrap_or("");
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }

    true
}

/// Superseded by `auth::password::validate_password_strength`; retained as a
/// test-only helper. No production callers.
#[cfg(test)]
pub fn is_strong_password(password: &str) -> bool {
    password.len() >= 8
}
#[cfg(test)]
mod tests {
    use super::*;
    // Test-only: the boundary assertions below pin the shared bound rather than
    // restating a literal, so widening the column can't leave them stale.
    use crate::modules::auth::username::DISPLAY_NAME_MAX_CHARS;


    const GOOD_PW: &str = "Str0ng-Pass!42";


    fn req(username: &str, email: &str, password: &str, display: Option<&str>) -> SetupAdminRequest {
        SetupAdminRequest {
            username: username.to_string(),
            email: email.to_string(),
            password: password.to_string(),
            display_name: display.map(|s| s.to_string()),
        }
    }


    #[test]
    fn validate_setup_request_accepts_a_well_formed_request() {
        assert!(validate_setup_request(&req("admin", "a@b.com", GOOD_PW, Some("Admin"))).is_ok());
        // display_name is optional.
        assert!(validate_setup_request(&req("admin", "a@b.com", GOOD_PW, None)).is_ok());
    }


    #[test]
    fn validate_setup_request_rejects_short_and_long_usernames() {
        assert!(validate_setup_request(&req("ab", "a@b.com", GOOD_PW, None)).is_err());
        let long = "a".repeat(101);
        assert!(validate_setup_request(&req(&long, "a@b.com", GOOD_PW, None)).is_err());
    }


    #[test]
    fn validate_setup_request_rejects_whitespace_and_control_chars_in_username() {
        assert!(validate_setup_request(&req("ad min", "a@b.com", GOOD_PW, None)).is_err());
        // A C0 control char (U+0007 BEL, Unicode category Cc) in the username
        // must be rejected — 13-misc F-06.
        assert!(
            validate_setup_request(&req("ad\u{0007}min", "a@b.com", GOOD_PW, None)).is_err(),
            "control char in username must be rejected"
        );
    }


    #[test]
    fn validate_setup_request_rejects_control_chars_in_display_name() {
        assert!(
            validate_setup_request(&req("admin", "a@b.com", GOOD_PW, Some("Ad\u{0007}min"))).is_err()
        );
        // The bound is now the shared one — `users.display_name` is
        // varchar(255) counted in CHARACTERS, not this path's old hand-rolled
        // 200 bytes. 255 fits the column and must be accepted; 256 must not.
        let at_bound = "x".repeat(DISPLAY_NAME_MAX_CHARS);
        assert!(validate_setup_request(&req("admin", "a@b.com", GOOD_PW, Some(&at_bound))).is_ok());
        let long_dn = "x".repeat(DISPLAY_NAME_MAX_CHARS + 1);
        assert!(validate_setup_request(&req("admin", "a@b.com", GOOD_PW, Some(&long_dn))).is_err());
    }


    #[test]
    fn validate_setup_request_rejects_weak_password() {
        assert!(validate_setup_request(&req("admin", "a@b.com", "short", None)).is_err());
    }


    #[test]
    fn is_valid_email_accepts_well_formed_addresses() {
        for e in ["a@b.com", "user.name@example.co", "x@sub.example.org"] {
            assert!(is_valid_email(e), "{e} should be valid");
        }
    }


    #[test]
    fn is_valid_email_rejects_malformed_addresses() {
        // The exact cases the strictened validator (13-misc F-05) closes.
        for e in [
            "", "a@.com", "a@b..com", "a@b.c.", "no-at-sign", "@b.com",
        ] {
            assert!(!is_valid_email(e), "{e} should be invalid");
        }
    }


    #[test]
    fn is_strong_password_enforces_min_length() {
        assert!(!is_strong_password("short"));
        assert!(is_strong_password("longenough"));
    }


    #[test]
    fn validate_setup_accepts_a_well_formed_request() {
        assert!(validate_setup_request(&req("rootadmin", "root@example.com", GOOD_PW, Some("Root"))).is_ok());
    }


    #[test]
    fn validate_setup_rejects_bad_usernames() {
        // Too short / too long.
        assert!(validate_setup_request(&req("ab", "a@b.co", GOOD_PW, None)).is_err());
        assert!(validate_setup_request(&req(&"x".repeat(101), "a@b.co", GOOD_PW, None)).is_err());
        // Whitespace + control chars (incl. RTL override U+202E spoofing).
        assert!(validate_setup_request(&req("root admin", "a@b.co", GOOD_PW, None)).is_err());
        assert!(validate_setup_request(&req("root\u{202E}admin", "a@b.co", GOOD_PW, None)).is_err());
    }


    #[test]
    fn validate_setup_rejects_long_display_name_and_bad_email_and_weak_password() {
        assert!(validate_setup_request(&req("rootadmin", "a@b.co", GOOD_PW, Some(&"d".repeat(DISPLAY_NAME_MAX_CHARS + 1)))).is_err());
        assert!(validate_setup_request(&req("rootadmin", "not-an-email", GOOD_PW, None)).is_err());
        assert!(validate_setup_request(&req("rootadmin", "a@b.co", "weak", None)).is_err());
    }


    #[test]
    fn is_valid_email_edge_cases() {
        assert!(is_valid_email("user@example.com"));
        assert!(!is_valid_email("noatsign"));
        assert!(!is_valid_email("a@@b.com"));
        assert!(!is_valid_email("a@.com")); // leading dot in domain
        assert!(!is_valid_email("a@b..com")); // consecutive dots
        assert!(!is_valid_email("a@bcom")); // no dot in domain
        assert!(!is_valid_email("a@b.c")); // TLD < 2 chars
        assert!(!is_valid_email("a b@c.com")); // whitespace
        assert!(!is_valid_email("a@-b.com")); // label starts with hyphen
    }


    #[test]
    fn email_validator_accepts_well_formed() {
        for ok in ["a@b.com", "user.name@sub.example.co", "x@y.io"] {
            assert!(is_valid_email(ok), "{ok} should be valid");
        }
    }


    #[test]
    fn email_validator_rejects_the_audited_malformed_cases() {
        // The exact F-05 failures the strictened validator closes.
        for bad in [
            "a@.com",      // leading-dot domain
            "a@b..com",    // consecutive dots
            "a@b.c.",      // trailing dot
            "a@b",         // no dot in domain
            "@b.com",      // empty local
            "a@",          // empty domain
            "a@@b.com",    // two @
            "a b@c.com",   // whitespace
            "a@b.c",       // 1-char TLD
            "a@b.c1",      // non-alpha TLD
            "",            // empty
        ] {
            assert!(!is_valid_email(bad), "{bad:?} should be rejected");
        }
    }


    #[test]
    fn setup_request_rejects_bad_username() {
        // Too short / too long.
        assert!(validate_setup_request(&req("ab", "a@b.com", "password123", None)).is_err());
        assert!(validate_setup_request(&req(&"x".repeat(101), "a@b.com", "password123", None)).is_err());
        // Whitespace + control chars (incl. RTL override U+202E spoofing).
        assert!(validate_setup_request(&req("ad min", "a@b.com", "password123", None)).is_err());
        assert!(validate_setup_request(&req("admin\u{202e}", "a@b.com", "password123", None)).is_err());
    }


    #[test]
    fn setup_request_display_name_control_and_length_gates() {
        // Control char in display name.
        assert!(validate_setup_request(&req("admin", "a@b.com", "password123", Some("ev\u{0007}il"))).is_err());
        // Over-length display name.
        assert!(validate_setup_request(&req("admin", "a@b.com", "password123", Some(&"x".repeat(DISPLAY_NAME_MAX_CHARS + 1)))).is_err());
        // A clean request passes all gates.
        assert!(validate_setup_request(&req("admin", "a@b.com", "password123", Some("Admin User"))).is_ok());
    }
}
