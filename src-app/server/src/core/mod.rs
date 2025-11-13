// Core functionality - event bus, config, module registry, database
// Week 1 implementation

pub mod app_builder;
pub mod app_state;
pub mod config;
pub mod database;
pub mod events;
pub mod repository_factory;

// Re-export commonly used functions
pub use app_state::{get_app_data_dir, set_app_data_dir};
pub use events::{AppEvent, EventBus, EventHandler};
pub use repository_factory::{init_repositories, Repos};
