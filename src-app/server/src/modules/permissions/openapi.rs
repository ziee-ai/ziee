// Chunk B3: `with_permission` + the `PermissionError` / `PermissionErrorDetails`
// / `PermissionDetail` 403-body schema types moved verbatim into
// `ziee_framework::permissions::openapi` (generic over `PermissionList`). This
// module is now an equivalence-preserving re-export shim so every
// `crate::modules::permissions::{with_permission, openapi::*}` call site — and
// the emitted OpenAPI 403 schema (schemars keys by the type's short name, so the
// names are unchanged) — is byte-identical. The moved `with_permission` unit
// test lives alongside the function in `ziee-framework`.

// The `PermissionError*` schema types are re-exported to preserve this module's
// former public surface (equivalence); ziee itself references only
// `with_permission` — the 403 schema is registered into the OpenAPI doc from
// inside the moved framework function, so ziee need not name the types.
#[allow(unused_imports)]
pub use ziee_framework::permissions::openapi::{
    PermissionDetail, PermissionError, PermissionErrorDetails, with_permission,
};
