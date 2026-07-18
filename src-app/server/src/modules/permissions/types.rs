// Permission system infrastructure
//
// Chunk B1b: the `PermissionCheck` / `PermissionList` traits and the
// `PermissionInfo` projection were moved verbatim into `ziee-identity`
// (identity abstractions, pluggable per decision #1). This module is now an
// equivalence-preserving re-export shim so every `crate::modules::permissions::
// {PermissionCheck, PermissionList, PermissionInfo}` call site is unchanged.
// The moved unit tests live alongside the traits in `ziee-identity`.

pub use ziee_identity::{PermissionCheck, PermissionInfo};
// `PermissionList`'s last by-name consumer in the binary's module tree was the
// sync module's `Audience::{all_of, any_of}` constructors, which moved to
// `ziee_framework::sync` in chunk B5 (the tuple `PermissionList` impls are used
// via trait resolution, not by name). It stays re-exported for API parity —
// `crate::PermissionList` (lib) + the `server_update` `#[cfg(test)]` call site
// still name it — so tolerate it being unused in a non-test `--bin` build.
#[allow(unused_imports)]
pub use ziee_identity::PermissionList;
