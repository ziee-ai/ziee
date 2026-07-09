//! Run-level caps for one `run_js` invocation (DEC-10). `JsLimits` (in
//! `runtime`) covers the in-interpreter caps (memory/stack/gas/output/console);
//! `JsCaps` adds the orchestration-level caps the executor enforces
//! (max tool calls, active-execution wall-clock, per-approval timeout).

use std::time::Duration;

use super::runtime::JsLimits;

#[derive(Clone, Debug)]
pub struct JsCaps {
    /// In-interpreter caps (memory / stack / gas / output / console).
    pub runtime: JsLimits,
    /// Max sub-tool calls one script may make (over-cap → the host fn throws).
    pub max_tool_calls: u64,
    /// Active-execution wall-clock backstop (EXCLUDES time spent awaiting an
    /// approval — the executor's watchdog pauses while any approval is pending).
    pub wall: Duration,
    /// How long a single per-call approval waits before resolving as cancel.
    pub approval_timeout: Duration,
}

impl Default for JsCaps {
    fn default() -> Self {
        Self {
            runtime: JsLimits::default(),
            max_tool_calls: 100,
            wall: Duration::from_secs(300),
            approval_timeout: Duration::from_secs(300),
        }
    }
}
