use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::modules::code_sandbox::types::CodeSandboxState;

static STATE: OnceCell<Arc<CodeSandboxState>> = OnceCell::new();

/// Set the global sandbox state. Called once at `code_sandbox::init()`.
/// Returns the existing state if already initialized.
pub fn init_state(state: CodeSandboxState) -> Arc<CodeSandboxState> {
    let arc = Arc::new(state);
    let _ = STATE.set(arc.clone());
    STATE.get().cloned().unwrap_or(arc)
}

/// Get the global sandbox state. Returns `None` until `init_state` has
/// been called (i.e. when `code_sandbox.enabled = false` or before boot).
pub fn get_state() -> Option<Arc<CodeSandboxState>> {
    STATE.get().cloned()
}
