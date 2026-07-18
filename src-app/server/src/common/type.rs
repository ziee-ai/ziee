use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// =====================================================
// API Result Type + Error Types
// =====================================================
//
// `ApiResult`, `ApiError`, and `AppError` (+ all their impls and the redaction
// regression tests) were moved into `ziee-core` in Chunk B1 of the SDK
// extraction. They are re-exported here (decision N2 — equivalence-preserving
// re-export shim) so the ~323 `crate::common::AppError` / `crate::common::type`
// call sites keep compiling byte-for-byte unchanged. AppError is not part of the
// OpenAPI surface (no `JsonSchema`), so the generated `types.ts` is unaffected.
// `ApiError` is not re-exported: it was only used internally by
// `AppError::into_response` (now in ziee-core), so re-exporting it here would be
// an unused import under the workspace's `unused_imports = "deny"` lint.
pub use ziee_core::{ApiResult, AppError};

// =====================================================
// Common Types
// =====================================================

/// Maximum page size accepted from the client. Larger values are clamped
/// silently at deserialization to prevent DoS via unbounded result-set
/// materialization (listing every user / file / message in one request).
/// Closes 03-user F-06 (Medium).
pub const PAGINATION_MAX_PER_PAGE: i32 = 100;

/// Shared default page size for list endpoints that bound an otherwise
/// unbounded query but don't take an explicit caller-supplied page size.
/// One source of truth so every such endpoint agrees on the cap.
pub const DEFAULT_PAGE_SIZE: i32 = 100;

/// Pagination query that clamps at deserialize time so every existing
/// handler that consumes `params.page` and `params.per_page` is safe
/// without touching its body.
///
/// - `page < 1` → 1 (prevents `(page-1)*per_page` underflow / negative offset)
/// - `per_page < 1` → 1 (prevents divide-by-zero in callers like
///   `total / per_page`)
/// - `per_page > PAGINATION_MAX_PER_PAGE` → PAGINATION_MAX_PER_PAGE
///   (prevents DoS)
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: i32,
    #[serde(default = "default_per_page")]
    pub per_page: i32,
}

impl<'de> Deserialize<'de> for PaginationQuery {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default = "default_page")]
            page: i32,
            #[serde(default = "default_per_page")]
            per_page: i32,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(PaginationQuery {
            page: if raw.page < 1 { 1 } else { raw.page },
            per_page: if raw.per_page < 1 {
                1
            } else if raw.per_page > PAGINATION_MAX_PER_PAGE {
                PAGINATION_MAX_PER_PAGE
            } else {
                raw.per_page
            },
        })
    }
}

impl PaginationQuery {
    /// `page` is already clamped on deserialize; this method is kept as
    /// an explicit no-op for callers built before the deserializer change.
    #[allow(dead_code)]
    pub fn page_clamped(&self) -> i32 {
        self.page
    }

    /// `per_page` is already clamped on deserialize; this method is kept
    /// as an explicit no-op for callers built before the deserializer change.
    #[allow(dead_code)]
    pub fn per_page_clamped(&self) -> i32 {
        self.per_page
    }
}

fn default_page() -> i32 {
    1
}

fn default_per_page() -> i32 {
    20
}

impl Default for PaginationQuery {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: The `AppError` redaction regression tests (database_error /
    // internal_with_id / From<sqlx::Error> / From<Box<dyn Error>> / trace_id /
    // not_found) moved with `AppError` into `ziee-core` (`error::tests`) in
    // Chunk B1. Only the `PaginationQuery` tests remain here.

    // =====================================================
    // PaginationQuery clamping — close 03-user F-06
    // =====================================================

    #[test]
    fn pagination_clamps_per_page_zero_on_deserialize() {
        let q: PaginationQuery = serde_json::from_str(r#"{"page":1,"per_page":0}"#).unwrap();
        assert_eq!(q.per_page, 1, "per_page=0 must clamp to 1 to prevent /0");
    }

    #[test]
    fn pagination_clamps_per_page_negative_on_deserialize() {
        let q: PaginationQuery = serde_json::from_str(r#"{"page":1,"per_page":-5}"#).unwrap();
        assert_eq!(q.per_page, 1);
    }

    #[test]
    fn pagination_clamps_per_page_oversized_on_deserialize() {
        let q: PaginationQuery = serde_json::from_str(r#"{"page":1,"per_page":10000}"#).unwrap();
        assert_eq!(q.per_page, PAGINATION_MAX_PER_PAGE);
    }

    #[test]
    fn pagination_clamps_page_negative_on_deserialize() {
        let q: PaginationQuery = serde_json::from_str(r#"{"page":-1,"per_page":20}"#).unwrap();
        assert_eq!(q.page, 1);
    }

    #[test]
    fn pagination_passes_through_valid_values_on_deserialize() {
        let q: PaginationQuery = serde_json::from_str(r#"{"page":5,"per_page":50}"#).unwrap();
        assert_eq!(q.page, 5);
        assert_eq!(q.per_page, 50);
    }

    #[test]
    fn pagination_defaults_when_fields_missing() {
        let q: PaginationQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q.page, 1);
        assert_eq!(q.per_page, 20);
    }
}
