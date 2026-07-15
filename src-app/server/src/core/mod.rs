// Core functionality - event bus, config, module registry, database
// Week 1 implementation

pub mod app_builder;
pub mod app_state;
pub mod config;
pub mod database;
pub mod events;
pub mod outbound;
pub mod repository;
pub mod secrets;
pub mod seed;

// Re-export commonly used functions
// set_server_addr is used from main.rs (binary target); suppress the
// unused-imports warning since it only appears in the library target.
#[allow(unused_imports)]
pub use app_state::{
    file_upload_body_limit_bytes, get_app_data_dir, get_caches_config, get_max_file_upload_bytes,
    get_server_addr, set_app_data_dir, set_caches_config, set_max_file_upload_bytes,
    set_server_addr,
};
pub use events::{AppEvent, EventBus, EventHandler};
pub use repository::{Repos, init_repositories, is_repos_initialized};
