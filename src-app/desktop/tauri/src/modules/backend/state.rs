//! Backend State Management
//!
//! Shared state for backend server status

use std::sync::{Arc, Mutex};

/// Shared state for the embedded backend server
#[derive(Clone)]
pub struct BackendState {
    /// The port the backend is running on
    port: u16,
    /// Whether the backend is ready to accept requests
    ready: Arc<Mutex<bool>>,
}

impl BackendState {
    /// Create a new backend state with the given port
    pub fn new(port: u16) -> Self {
        Self {
            port,
            ready: Arc::new(Mutex::new(false)),
        }
    }

    /// Get the backend server port
    pub fn get_port(&self) -> u16 {
        self.port
    }

    /// Check if the backend is ready
    #[allow(dead_code)]
    pub fn is_ready(&self) -> bool {
        *self.ready.lock().unwrap()
    }

    /// Set the ready state
    pub fn set_ready(&self, ready: bool) {
        *self.ready.lock().unwrap() = ready;
    }
}
