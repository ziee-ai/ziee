//! Common test utilities for desktop integration tests

pub mod server;

use std::net::TcpListener;

/// Find an available port in the given range
pub fn find_available_port(start: u16, end: u16) -> Option<u16> {
    (start..end).find(|port| TcpListener::bind(("127.0.0.1", *port)).is_ok())
}

/// Test configuration for desktop app tests
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub server_port: u16,
    pub data_dir: std::path::PathBuf,
}

impl TestConfig {
    pub fn new(data_dir: std::path::PathBuf) -> Self {
        let server_port = find_available_port(18080, 18180).expect("No available port found");
        Self {
            server_port,
            data_dir,
        }
    }
}
