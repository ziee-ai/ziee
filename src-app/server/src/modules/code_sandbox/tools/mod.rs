//! Tool implementations dispatched by `handlers::jsonrpc_handler`.
//! `execute` moved to the `ziee_sandbox` engine (re-exported here so every
//! `crate::modules::code_sandbox::tools::execute::…` path resolves unchanged);
//! `files` (DB-backed workspace file ops) stays in the ziee server crate.

pub use ziee_sandbox::tools::execute;
pub mod files;
