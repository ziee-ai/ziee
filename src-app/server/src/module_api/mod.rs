// Module API - trait definitions for AppModule
// Week 1, Day 1 implementation

pub mod backend_module;
pub mod cli_module;
pub mod types;

pub use backend_module::{AppModule, ModuleContext};
pub use cli_module::{CliEntry, CLI_ENTRIES};
pub use types::*;
