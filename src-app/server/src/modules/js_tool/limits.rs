//! Run-level caps for one `run_js` invocation (DEC-10). `JsLimits` (in
//! `runtime`) covers the in-interpreter caps (memory/stack/gas/output/console);
//! `JsCaps` adds the orchestration-level caps the executor enforces
//! (max tool calls, active-execution wall-clock, per-approval timeout).

use std::time::Duration;

use super::runtime::JsLimits;
use super::settings::JsToolSettings;

#[derive(Clone, Debug)]
pub struct JsCaps {
    /// In-interpreter caps (memory / stack / gas / output / console).
    pub runtime: JsLimits,
    /// Max sub-tool calls one script may make (over-cap → the host fn throws).
    pub max_tool_calls: u64,
    /// Max per-call approval prompts one script may raise. Bounds cumulative
    /// suspended time (max_approvals × approval_timeout) so a script can't hold a
    /// runtime for hours by spamming approvals the user ignores.
    pub max_approvals: u64,
    /// Active-execution wall-clock backstop (EXCLUDES time spent awaiting an
    /// approval — the executor's watchdog pauses while any approval is pending).
    pub wall: Duration,
    /// How long a single per-call approval waits before resolving as cancel.
    pub approval_timeout: Duration,
    /// Max sub-tool dispatches a single run may have in flight at once (the
    /// per-run dispatch semaphore). Admin-tunable (DEC-23).
    pub max_concurrent_dispatch: usize,
    /// Max per-sub-call trace entries retained for the result's
    /// `structured_content.tool_calls`. Admin-tunable (DEC-23).
    pub max_trace_entries: usize,
}

impl Default for JsCaps {
    fn default() -> Self {
        Self {
            runtime: JsLimits::default(),
            max_tool_calls: 100,
            max_approvals: 25,
            wall: Duration::from_secs(300),
            approval_timeout: Duration::from_secs(300),
            max_concurrent_dispatch: 6,
            max_trace_entries: 256,
        }
    }
}

impl JsCaps {
    /// Build the per-run caps from the admin-configurable settings row (the 7
    /// tunable fields — DEC-20). `gas`/`output_bytes`/`console_bytes` (JsLimits)
    /// and `max_tool_calls`/`max_approvals` are NOT settings-driven and keep
    /// their defaults. The settings values are DB/`validate()`-bounded, so the
    /// `i64/i32 → usize` conversions are in range on a 64-bit target.
    pub fn from_settings(s: &JsToolSettings) -> Self {
        let defaults = JsLimits::default();
        Self {
            runtime: JsLimits {
                memory_bytes: s.memory_bytes.max(1) as usize,
                max_stack_bytes: s.max_stack_bytes.max(1) as usize,
                ..defaults
            },
            max_tool_calls: 100,
            max_approvals: 25,
            wall: Duration::from_secs(s.wall_secs.max(1) as u64),
            approval_timeout: Duration::from_secs(s.approval_timeout_secs.max(1) as u64),
            max_concurrent_dispatch: s.max_concurrent_dispatch.max(1) as usize,
            max_trace_entries: s.max_trace_entries.max(1) as usize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::js_tool::settings_cache;

    // TEST-39: from_settings maps the 7 tunable fields; leaves the rest at defaults.
    #[test]
    fn from_settings_maps_tunables_and_keeps_defaults() {
        let mut s = settings_cache::defaults();
        s.memory_bytes = 32 * 1024 * 1024;
        s.max_stack_bytes = 256 * 1024;
        s.wall_secs = 42;
        s.approval_timeout_secs = 17;
        s.max_concurrent_dispatch = 3;
        s.max_trace_entries = 99;
        let caps = JsCaps::from_settings(&s);
        assert_eq!(caps.runtime.memory_bytes, 32 * 1024 * 1024);
        assert_eq!(caps.runtime.max_stack_bytes, 256 * 1024);
        assert_eq!(caps.wall, Duration::from_secs(42));
        assert_eq!(caps.approval_timeout, Duration::from_secs(17));
        assert_eq!(caps.max_concurrent_dispatch, 3);
        assert_eq!(caps.max_trace_entries, 99);
        // Not settings-driven → defaults preserved.
        let d = JsLimits::default();
        assert_eq!(caps.runtime.gas, d.gas);
        assert_eq!(caps.runtime.output_bytes, d.output_bytes);
        assert_eq!(caps.runtime.console_bytes, d.console_bytes);
        assert_eq!(caps.max_tool_calls, 100);
        assert_eq!(caps.max_approvals, 25);
    }
}
