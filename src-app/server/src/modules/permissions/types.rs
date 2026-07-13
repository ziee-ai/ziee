// Permission system infrastructure
//
// Chunk B1b: the `PermissionCheck` / `PermissionList` traits and the
// `PermissionInfo` projection were moved verbatim into `ziee-identity`
// (identity abstractions, pluggable per decision #1). This module is now an
// equivalence-preserving re-export shim so every `crate::modules::permissions::
// {PermissionCheck, PermissionList, PermissionInfo}` call site is unchanged.
// The moved unit tests live alongside the traits in `ziee-identity`.

pub use ziee_identity::{PermissionCheck, PermissionInfo, PermissionList};
