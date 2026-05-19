use once_cell::sync::OnceCell;

use super::types::CodeSandboxState;

static SANDBOX_CONFIG: OnceCell<CodeSandboxState> = OnceCell::new();

pub fn init_sandbox_config(state: CodeSandboxState) {
    SANDBOX_CONFIG
        .set(state)
        .expect("sandbox config already initialized");
}

pub fn get_sandbox_config() -> &'static CodeSandboxState {
    SANDBOX_CONFIG
        .get()
        .expect("sandbox config not initialized")
}
