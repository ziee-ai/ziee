//! LlamaCpp engine binary builder
//!
//! This crate builds llama-server from llama.cpp source with comprehensive
//! platform and GPU backend support.

use std::path::PathBuf;

/// Get the path to the built llama-server binary
///
/// The binary path is determined at compile time by the build script,
/// which builds llama.cpp with appropriate platform and GPU backend configuration.
///
/// # Panics
///
/// Panics if the binary was not built successfully (should only happen at compile time).
pub fn binary_path() -> PathBuf {
    PathBuf::from(env!("LLAMACPP_BINARY_PATH"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_path_exists() {
        let path = binary_path();
        assert!(
            path.exists(),
            "llama-server binary should exist at: {}",
            path.display()
        );
        assert!(
            path.is_file(),
            "llama-server path should be a file: {}",
            path.display()
        );
    }
}
