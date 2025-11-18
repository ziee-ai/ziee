// Module API - trait definitions for AppModule
// Week 1, Day 1 implementation

use sqlx::PgPool;
use std::sync::Arc;

pub mod backend_module;
pub mod types;

pub use backend_module::{AppModule, ModuleContext};
pub use types::*;

/// Database pool type used across modules
pub type DbPool = Arc<PgPool>;
