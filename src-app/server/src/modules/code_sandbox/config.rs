use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::modules::code_sandbox::types::CodeSandboxState;

static STATE: OnceCell<Arc<CodeSandboxState>> = OnceCell::new();

/// Set the global sandbox state. Called once at `code_sandbox::init()`.
/// Returns the existing state if already initialized; the second call
/// is logged at WARN level so test harnesses / hot-reload paths see a
/// clear signal that the new state was discarded.
pub fn init_state(state: CodeSandboxState) -> Arc<CodeSandboxState> {
    let arc = Arc::new(state);
    if let Err(_) = STATE.set(arc.clone()) {
        tracing::warn!(
            "code_sandbox::init_state called more than once; \
             second call's state is discarded and the FIRST state \
             remains in effect. This typically happens in test \
             harnesses; in production it indicates a double init()."
        );
    }
    STATE.get().cloned().unwrap_or(arc)
}

/// Get the global sandbox state. Returns `None` until `init_state` has
/// been called (i.e. when `code_sandbox.enabled = false` or before boot).
pub fn get_state() -> Option<Arc<CodeSandboxState>> {
    STATE.get().cloned()
}
