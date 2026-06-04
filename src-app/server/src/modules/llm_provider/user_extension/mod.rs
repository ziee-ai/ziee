// llm_provider's bridge into the user/Group domain.
//
// Owns the `user_group_llm_providers` join-table surface:
//   - 5 HTTP routes under `/llm-providers/{provider_id}/groups*` and
//     `/groups/{group_id}/providers*`
//   - 5 handlers + 5 OpenAPI docs siblings (preserve `.id()` strings —
//     they're the frontend autogen-client method names)
//   - `Repos.user_group_llm_provider` repo with 6 methods (the 5
//     handler-backed ones plus `user_has_access_to_provider`, used by
//     `chat/core/handlers/streaming.rs` for the per-message access
//     check)
//
// Acid test: `grep -rnE "use crate::modules::user" llm_provider/ |
// grep -v user_extension/` returns empty. The `use
// crate::modules::user::models::Group` import is concentrated here.

pub mod handlers;
pub mod repository;
pub mod routes;

pub use repository::UserGroupLlmProviderRepository;
