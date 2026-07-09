//! Pure embedded-QuickJS evaluation for `run_js` — NO chat/DB context.
//!
//! Responsibilities:
//! - Build an `AsyncRuntime`/`AsyncContext` with the caps applied
//!   (`set_memory_limit`, `set_max_stack_size`, an interrupt handler over a
//!   shared cancel-flag + gas counter for CPU/loop kill).
//! - Install `console.*` capture and a result channel.
//! - Let the caller inject host functions onto a `ziee` global (the
//!   `host_bridge` does this; the pure unit tests pass a no-op).
//! - Evaluate the model script wrapped as an async IIFE, drive the runtime to
//!   quiescence, and return `{ value, console, error{message,line} }`.
//!
//! The interrupt handler bounds only *synchronous JS execution* (it fires on
//! bytecode back-edges/calls, never while JS is suspended awaiting a Rust host
//! future) — so a legitimate minutes-long approval-wait never trips it. The
//! outer wall-clock is enforced by the caller (`executor`) by tripping the
//! shared `cancel` flag, which the interrupt handler observes on the next JS
//! instruction.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rquickjs::prelude::Func;
use rquickjs::{AsyncContext, AsyncRuntime, CatchResultExt, Ctx, Object};

/// Caps applied to a single `run_js` evaluation. Defaults mirror DEC-10.
#[derive(Clone, Debug)]
pub struct JsLimits {
    /// `set_memory_limit` (bytes). Never 0 (0 = unlimited in QuickJS).
    pub memory_bytes: usize,
    /// `set_max_stack_size` (bytes).
    pub max_stack_bytes: usize,
    /// Interrupt-handler ticks permitted before a pure-CPU runaway is killed.
    /// The handler is polled on bytecode back-edges/calls, so this bounds
    /// synchronous execution work independent of any await duration.
    pub gas: u64,
    /// Cap on the JSON-serialized final value (bytes); over-cap → truncated.
    pub output_bytes: usize,
    /// Cap on total captured console output (bytes); over-cap → dropped.
    pub console_bytes: usize,
}

impl Default for JsLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 128 * 1024 * 1024,
            max_stack_bytes: 512 * 1024,
            // ~ a few million back-edges: kills `while(true){}` in well under a
            // second of solid CPU while leaving ample headroom for real scripts.
            gas: 8_000_000,
            output_bytes: 128 * 1024,
            console_bytes: 64 * 1024,
        }
    }
}

/// A structured JS error surfaced back to the model for one-shot self-correction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsError {
    pub message: String,
    /// Best-effort 1-based source line parsed from the exception stack.
    pub line: Option<u32>,
}

/// Outcome of one evaluation. Exactly one of `value` / `error` is meaningful:
/// `error.is_some()` means the script threw or was killed.
#[derive(Clone, Debug)]
pub struct JsOutcome {
    /// The script's resolved value (already output-capped). `Null` on error.
    pub value: serde_json::Value,
    pub console: Vec<String>,
    pub error: Option<JsError>,
    /// True if the final value was truncated to `output_bytes`.
    pub truncated_output: bool,
}

/// Wrap a model script as an async IIFE so `await`/`return` work and the
/// resolved value is captured via `__ziee_set_result`. Uncaught throws are
/// serialized to `{err,stack}`; a normal completion to `{ok}`.
fn wrap_script(script: &str) -> String {
    // The inner IIFE is the user's body; the outer one try/catches it and
    // reports exactly one result. `undefined` collapses to JSON `null`.
    format!(
        r#"(async () => {{
  try {{
    const __r = await (async () => {{
{script}
    }})();
    __ziee_set_result(JSON.stringify({{ ok: __r === undefined ? null : __r }}));
  }} catch (e) {{
    __ziee_set_result(JSON.stringify({{
      err: (e && e.message) ? String(e.message) : String(e),
      stack: (e && e.stack) ? String(e.stack) : ""
    }}));
  }}
}})();
"#
    )
}

/// The JS prelude wiring `console.*` to the Rust capture sink `__ziee_log`.
const CONSOLE_PRELUDE: &str = r#"
globalThis.console = {
  log:   (...a) => __ziee_log(a.map(x => { try { return typeof x === 'string' ? x : JSON.stringify(x); } catch (_) { return String(x); } }).join(' ')),
};
globalThis.console.info = globalThis.console.log;
globalThis.console.warn = globalThis.console.log;
globalThis.console.error = globalThis.console.log;
globalThis.console.debug = globalThis.console.log;
"#;

/// Parse a best-effort 1-based line number out of a QuickJS exception stack.
/// QuickJS frames look like `    at <anonymous> (eval_script:12)` — we take the
/// first integer following the last colon on a frame line.
fn line_from_stack(stack: &str) -> Option<u32> {
    for frame in stack.lines() {
        if let Some(colon) = frame.rfind(':') {
            let tail: String = frame[colon + 1..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(n) = tail.parse::<u32>() {
                if n > 0 {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Evaluate `script` in a fresh capped interpreter. `inject` runs inside the
/// context and may populate the `ziee` global object with host functions; the
/// pure unit tests pass a no-op closure.
///
/// `cancel` is shared with the caller: setting it makes the interrupt handler
/// kill the script on the next JS instruction (the executor uses this for the
/// wall-clock backstop). The handler ALSO trips when `gas` is exhausted.
pub async fn evaluate<F>(script: &str, limits: &JsLimits, cancel: Arc<AtomicBool>, inject: F) -> JsOutcome
where
    F: for<'js> FnOnce(&Ctx<'js>) -> rquickjs::Result<()> + Send,
{
    let rt = match AsyncRuntime::new() {
        Ok(rt) => rt,
        Err(e) => {
            return JsOutcome {
                value: serde_json::Value::Null,
                console: Vec::new(),
                error: Some(JsError { message: format!("runtime init failed: {e}"), line: None }),
                truncated_output: false,
            };
        }
    };
    rt.set_memory_limit(limits.memory_bytes).await;
    rt.set_max_stack_size(limits.max_stack_bytes).await;

    // Interrupt handler: gas-metered CPU kill + observing the shared cancel flag.
    // `gas` counts down on each poll; hitting zero (or a set cancel) returns
    // `true`, which QuickJS turns into an uncatchable interruption.
    let gas = Arc::new(AtomicU64::new(limits.gas));
    {
        let gas = gas.clone();
        let cancel = cancel.clone();
        rt.set_interrupt_handler(Some(Box::new(move || {
            if cancel.load(Ordering::Relaxed) {
                return true;
            }
            // saturating decrement; kill when exhausted
            let prev = gas.fetch_sub(1, Ordering::Relaxed);
            prev == 0
        })))
        .await;
    }

    let ctx = match AsyncContext::full(&rt).await {
        Ok(ctx) => ctx,
        Err(e) => {
            return JsOutcome {
                value: serde_json::Value::Null,
                console: Vec::new(),
                error: Some(JsError { message: format!("context init failed: {e}"), line: None }),
                truncated_output: false,
            };
        }
    };

    // Shared sinks. `Arc`/atomics (not `Rc`/`Cell`) so the closures capturing
    // them are `Send` — required because `parallel` gives the runtime its `Send`
    // marker and the `ctx.with` closure must then be `Send`. The runtime is
    // still single-threaded (one driving task), so there is no real contention.
    let console: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let console_bytes = Arc::new(AtomicUsize::new(0));
    let console_dropped = Arc::new(AtomicBool::new(false));
    let result: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let console_cap = limits.console_bytes;

    // Phase 1: install host env + kick off the wrapped script.
    let wrapped = wrap_script(script);
    let eval_err: Option<JsError> = ctx
        .with({
            let console = console.clone();
            let console_bytes = console_bytes.clone();
            let console_dropped = console_dropped.clone();
            let result = result.clone();
            move |ctx| {
                let globals = ctx.globals();

                // __ziee_log(String): append to the console sink under the cap.
                let log_console = console.clone();
                let log_bytes = console_bytes.clone();
                let log_dropped = console_dropped.clone();
                globals.set(
                    "__ziee_log",
                    Func::from(move |line: String| {
                        let cur = log_bytes.load(Ordering::Relaxed);
                        if cur >= console_cap {
                            log_dropped.store(true, Ordering::Relaxed);
                            return;
                        }
                        let remaining = console_cap - cur;
                        let piece = if line.len() > remaining {
                            log_dropped.store(true, Ordering::Relaxed);
                            line.chars().take(remaining).collect::<String>()
                        } else {
                            line
                        };
                        log_bytes.store(cur + piece.len(), Ordering::Relaxed);
                        if let Ok(mut c) = log_console.lock() {
                            c.push(piece);
                        }
                    }),
                )?;

                // __ziee_set_result(String): capture the final JSON payload.
                let set_result = result.clone();
                globals.set(
                    "__ziee_set_result",
                    Func::from(move |payload: String| {
                        // First writer wins (the wrapper calls it exactly once).
                        if let Ok(mut slot) = set_result.lock() {
                            if slot.is_none() {
                                *slot = Some(payload);
                            }
                        }
                    }),
                )?;

                // console shim.
                ctx.eval::<(), _>(CONSOLE_PRELUDE)?;

                // Caller-injected host functions. Set an empty `ziee` global
                // FIRST so an injector may eval a JS prelude that references
                // `globalThis.ziee` (host_bridge does).
                let ziee = Object::new(ctx.clone())?;
                globals.set("ziee", ziee)?;
                inject(&ctx)?;

                // Kick off the script. A synchronous runaway (`while(true){}`
                // with no await) is interrupted HERE and surfaces as an Err.
                match ctx.eval::<(), _>(wrapped.as_bytes()).catch(&ctx) {
                    Ok(()) => Ok::<Option<JsError>, rquickjs::Error>(None),
                    Err(caught) => {
                        let msg = caught.to_string();
                        let line = line_from_stack(&msg);
                        Ok(Some(JsError { message: msg, line }))
                    }
                }
            }
        })
        .await
        .unwrap_or_else(|e| Some(JsError { message: format!("eval failed: {e}"), line: None }));

    // Phase 2: drive all pending jobs + async host futures to quiescence. A
    // runaway or OOM in an awaited section aborts the job here; caught JS throws
    // resolve via the wrapper's try/catch into `__ziee_set_result`.
    rt.idle().await;

    // Assemble the outcome.
    let console_out = {
        let mut v = console.lock().map(|c| c.clone()).unwrap_or_default();
        if console_dropped.load(Ordering::Relaxed) {
            v.push("[console output truncated]".to_string());
        }
        v
    };

    let captured = result.lock().ok().and_then(|r| r.clone());
    match captured {
        Some(payload) => {
            // payload is `{ok: <value>}` or `{err, stack}`.
            match serde_json::from_str::<serde_json::Value>(&payload) {
                Ok(v) if v.get("err").is_some() => {
                    let message = v
                        .get("err")
                        .and_then(|e| e.as_str())
                        .unwrap_or("script error")
                        .to_string();
                    let line = v.get("stack").and_then(|s| s.as_str()).and_then(line_from_stack);
                    JsOutcome {
                        value: serde_json::Value::Null,
                        console: console_out,
                        error: Some(JsError { message, line }),
                        truncated_output: false,
                    }
                }
                Ok(v) => {
                    let raw = v.get("ok").cloned().unwrap_or(serde_json::Value::Null);
                    let (value, truncated) = cap_output(raw, limits.output_bytes);
                    JsOutcome { value, console: console_out, error: None, truncated_output: truncated }
                }
                Err(e) => JsOutcome {
                    value: serde_json::Value::Null,
                    console: console_out,
                    error: Some(JsError { message: format!("result decode failed: {e}"), line: None }),
                    truncated_output: false,
                },
            }
        }
        // No result captured: an uncatchable interruption (CPU/gas or cancel) or
        // an OOM aborted the job before the wrapper could report. Prefer a
        // concrete eval error if we have one.
        None => {
            let err = eval_err.unwrap_or(JsError {
                message: if cancel.load(Ordering::Relaxed) {
                    "script terminated: wall-clock limit exceeded".to_string()
                } else {
                    "script terminated: CPU or memory limit exceeded, or it did not complete"
                        .to_string()
                },
                line: None,
            });
            JsOutcome { value: serde_json::Value::Null, console: console_out, error: Some(err), truncated_output: false }
        }
    }
}

/// Cap the serialized size of the final value. Over-cap → replace with a marker
/// carrying a size and a UTF-8-safe preview so the model still gets a signal.
fn cap_output(value: serde_json::Value, cap: usize) -> (serde_json::Value, bool) {
    let serialized = value.to_string();
    if serialized.len() <= cap {
        return (value, false);
    }
    let preview: String = serialized.chars().take(cap.min(2000)).collect();
    (
        serde_json::json!({
            "_truncated": true,
            "_bytes": serialized.len(),
            "preview": preview,
        }),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_inject<'js>(_c: &Ctx<'js>) -> rquickjs::Result<()> {
        Ok(())
    }

    async fn run(script: &str) -> JsOutcome {
        evaluate(script, &JsLimits::default(), Arc::new(AtomicBool::new(false)), no_inject).await
    }

    // TEST-1: dep + features + default allocator wired — a trivial eval works.
    #[tokio::test]
    async fn test_dep_wired_trivial_eval() {
        let out = run("return 1 + 1;").await;
        assert!(out.error.is_none(), "unexpected error: {:?}", out.error);
        assert_eq!(out.value, serde_json::json!(2));
    }

    // TEST-2: async IIFE wrapper awaits and returns the final value.
    #[tokio::test]
    async fn test_async_return_final_value() {
        let out = run("const x = await Promise.resolve(21); return x * 2;").await;
        assert!(out.error.is_none(), "unexpected error: {:?}", out.error);
        assert_eq!(out.value, serde_json::json!(42));
    }

    // TEST-3: console.* is captured in order and truncated at the cap.
    #[tokio::test]
    async fn test_console_capture_and_cap() {
        let out = run("console.log('a'); console.warn('b'); console.error('c'); return null;").await;
        assert!(out.error.is_none(), "unexpected error: {:?}", out.error);
        assert_eq!(&out.console[..3], &["a".to_string(), "b".to_string(), "c".to_string()]);

        // Over-cap → dropped + marker appended.
        let limits = JsLimits { console_bytes: 8, ..JsLimits::default() };
        let out = evaluate(
            "for (let i = 0; i < 100; i++) console.log('xxxx'); return null;",
            &limits,
            Arc::new(AtomicBool::new(false)),
            no_inject,
        )
        .await;
        assert!(out.console.iter().any(|l| l.contains("truncated")), "expected truncation marker: {:?}", out.console);
    }

    // TEST-4: a throwing script returns error{message, line}.
    #[tokio::test]
    async fn test_error_with_line() {
        let out = run("const a = 1;\nthrow new Error('boom');").await;
        let err = out.error.expect("expected an error");
        assert!(err.message.contains("boom"), "message: {}", err.message);
        assert!(err.line.is_some(), "expected a line number, stack had none");
    }

    // TEST-5: `while(true){}` is killed by the interrupt handler, not hung.
    #[tokio::test]
    async fn test_cpu_interrupt_kills_infinite_loop() {
        let limits = JsLimits { gas: 200_000, ..JsLimits::default() };
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            evaluate("while (true) {}", &limits, Arc::new(AtomicBool::new(false)), no_inject),
        )
        .await
        .expect("evaluate hung past the interrupt — CPU kill failed");
        let err = out.error.expect("expected a CPU-limit error");
        assert!(
            err.message.to_lowercase().contains("interrupt")
                || err.message.to_lowercase().contains("cpu")
                || err.message.to_lowercase().contains("did not complete"),
            "unexpected message: {}",
            err.message
        );
    }

    // TEST-6: exceeding the memory cap surfaces a memory error (cap is live
    // under the default allocator).
    #[tokio::test]
    async fn test_memory_limit_enforced() {
        let limits = JsLimits { memory_bytes: 1 * 1024 * 1024, ..JsLimits::default() };
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            evaluate(
                "const a = []; for (let i = 0; i < 100000000; i++) { a.push({i, s: 'padding-padding-padding'}); } return a.length;",
                &limits,
                Arc::new(AtomicBool::new(false)),
                no_inject,
            ),
        )
        .await
        .expect("evaluate hung — memory kill failed");
        assert!(out.error.is_some(), "expected a memory error, got value {:?}", out.value);
    }

    // TEST-7: an oversized final value is truncated to the output cap.
    #[tokio::test]
    async fn test_output_cap_truncates() {
        let limits = JsLimits { output_bytes: 64, ..JsLimits::default() };
        let out = evaluate("return 'x'.repeat(10000);", &limits, Arc::new(AtomicBool::new(false)), no_inject).await;
        assert!(out.error.is_none(), "unexpected error: {:?}", out.error);
        assert!(out.truncated_output, "expected truncation");
        assert_eq!(out.value.get("_truncated"), Some(&serde_json::json!(true)));
    }

    // TEST-37: NO ambient capability — no require/fetch/process/Deno, no fs/net;
    // only injected `ziee` exists.
    #[tokio::test]
    async fn test_no_ambient_capabilities() {
        let out = run(
            r#"return {
                require: typeof require,
                fetch: typeof fetch,
                process: typeof process,
                deno: typeof globalThis.Deno,
                xhr: typeof XMLHttpRequest,
                zieeIsObject: typeof ziee === 'object'
            };"#,
        )
        .await;
        assert!(out.error.is_none(), "unexpected error: {:?}", out.error);
        assert_eq!(out.value["require"], "undefined");
        assert_eq!(out.value["fetch"], "undefined");
        assert_eq!(out.value["process"], "undefined");
        assert_eq!(out.value["deno"], "undefined");
        assert_eq!(out.value["xhr"], "undefined");
        assert_eq!(out.value["zieeIsObject"], true);
    }

    // Cancel flag (wall-clock backstop) terminates a post-await runaway.
    // Needs a multi-thread runtime: the cancel-setter task must run on another
    // worker while `idle()` monopolizes a thread in the synchronous FFI loop
    // (production uses a multi-thread runtime + gas as the primary CPU guard).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_cancel_flag_terminates() {
        let cancel = Arc::new(AtomicBool::new(false));
        let c2 = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            c2.store(true, Ordering::Relaxed);
        });
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            evaluate(
                "await Promise.resolve(); while (true) {}",
                &JsLimits { gas: u64::MAX, ..JsLimits::default() },
                cancel,
                no_inject,
            ),
        )
        .await
        .expect("cancel did not terminate the script");
        assert!(out.error.is_some(), "expected termination error");
    }
}
