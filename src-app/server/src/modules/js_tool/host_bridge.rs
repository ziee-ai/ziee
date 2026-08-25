//! Injects the conversation's tools as async host functions on the `ziee`
//! global: `ziee.tools.<name>({...})`, `ziee.call(name, args)`, and
//! `ziee.toolList()`. Marshalling crosses the boundary as JSON strings
//! (DEC-19): the JS wrapper `JSON.stringify`s args into `__ziee_dispatch`, which
//! returns a JSON string the wrapper `JSON.parse`s.
//!
//! This file is decoupled from the MCP dispatcher: the caller (`executor`)
//! supplies a `DispatchFn`, so the friendly-name derivation and the call budget
//! stay unit-testable without a chat context.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rquickjs::function::Async;
use rquickjs::{Ctx, Function};
use uuid::Uuid;

/// A tool exposed to the script, with a collision-safe JS binding name.
#[derive(Clone, Debug)]
pub struct ToolBinding {
    /// Friendly, collision-safe identifier used as `ziee.tools.<js_name>`.
    pub js_name: String,
    pub server_id: Uuid,
    pub server_name: String,
    /// The real (unprefixed) MCP tool name passed to the dispatcher.
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Raw tool metadata (as assembled from the accessible-server list).
#[derive(Clone, Debug)]
pub struct RawTool {
    pub server_id: Uuid,
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Sanitize an arbitrary tool/server name to a valid JS identifier fragment.
fn sanitize(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if out.is_empty() {
        out.push('_');
    }
    if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        out.insert(0, '_');
    }
    out
}

/// Derive collision-safe `js_name`s: a tool keeps its own name when unique
/// across all attached servers; on collision it becomes `<server>_<tool>`; any
/// residual collision gets a numeric suffix. (TEST-8)
pub fn build_bindings(tools: &[RawTool]) -> Vec<ToolBinding> {
    // Count occurrences of each raw tool name to detect collisions.
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for t in tools {
        *counts.entry(t.tool_name.as_str()).or_insert(0) += 1;
    }

    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(tools.len());
    for t in tools {
        let base = if counts.get(t.tool_name.as_str()).copied().unwrap_or(0) > 1 {
            format!("{}_{}", sanitize(&t.server_name), sanitize(&t.tool_name))
        } else {
            sanitize(&t.tool_name)
        };
        // Resolve any residual collision deterministically.
        let mut js_name = base.clone();
        let mut n = 2;
        while used.contains(&js_name) {
            js_name = format!("{base}_{n}");
            n += 1;
        }
        used.insert(js_name.clone());
        out.push(ToolBinding {
            js_name,
            server_id: t.server_id,
            server_name: t.server_name.clone(),
            tool_name: t.tool_name.clone(),
            description: t.description.clone(),
            input_schema: t.input_schema.clone(),
        });
    }
    out
}

/// A bounded counter for the number of sub-tool calls a single script may make.
/// (TEST-10) Over-budget → the dispatcher returns an error the host fn throws.
#[derive(Clone)]
pub struct CallBudget {
    used: Arc<AtomicU64>,
    max: u64,
}
impl CallBudget {
    pub fn new(max: u64) -> Self {
        Self { used: Arc::new(AtomicU64::new(0)), max }
    }
    /// Try to claim one call slot. Returns false when the budget is exhausted.
    pub fn try_claim(&self) -> bool {
        // fetch_add then compare so concurrent claims can't both slip past.
        let prev = self.used.fetch_add(1, Ordering::SeqCst);
        prev < self.max
    }
    #[allow(dead_code)] // test-only + public API for future trace surfacing
    pub fn used(&self) -> u64 {
        self.used.load(Ordering::SeqCst).min(self.max)
    }
    pub fn max(&self) -> u64 {
        self.max
    }
}

/// The dispatch entry point supplied by `executor`. Given `(js_name, args)`, it
/// returns a JSON value that is EITHER `{ "value": <mcp-result> }` on success or
/// `{ "__error": "<message>" }` (thrown into the script as an `Error`).
pub type DispatchFn = Arc<
    dyn Fn(String, serde_json::Value) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>>
        + Send
        + Sync,
>;

/// The JS prelude that builds `ziee.call` / `ziee.tools` / `ziee.toolList` on top
/// of the injected `__ziee_dispatch` + `__ziee_toollist` globals.
const ZIEE_PRELUDE: &str = r#"
(() => {
  const z = globalThis.ziee;
  z.call = async (name, args) => {
    const raw = await __ziee_dispatch(String(name), JSON.stringify(args === undefined ? {} : args));
    const res = JSON.parse(raw);
    if (res && res.__error !== undefined && res.__error !== null) {
      const e = new Error(res.__error);
      e.name = 'ToolApprovalDenied';
      throw e;
    }
    return res.value;
  };
  z.toolList = () => JSON.parse(JSON.stringify(__ziee_toollist));
  z.tools = {};
  for (const t of __ziee_toollist) {
    z.tools[t.jsName] = (args) => z.call(t.jsName, args);
  }
})();
"#;

/// Install `__ziee_dispatch`, `__ziee_toollist`, and the `ziee.*` prelude onto
/// the context. Called from `runtime::evaluate`'s inject callback (the empty
/// `ziee` global already exists at this point; the prelude populates it).
pub fn install<'js>(
    ctx: &Ctx<'js>,
    bindings: &[ToolBinding],
    dispatch: DispatchFn,
) -> rquickjs::Result<()> {
    // __ziee_dispatch(name, args_json) -> Promise<result_json>
    let f = Function::new(
        ctx.clone(),
        Async(move |name: String, args_json: String| {
            let dispatch = dispatch.clone();
            async move {
                let args: serde_json::Value =
                    serde_json::from_str(&args_json).unwrap_or_else(|_| serde_json::json!({}));
                let out = dispatch(name, args).await;
                Ok::<String, rquickjs::Error>(out.to_string())
            }
        }),
    )?;
    ctx.globals().set("__ziee_dispatch", f)?;

    // __ziee_toollist: [{ jsName, name, description, inputSchema }]
    let list: Vec<serde_json::Value> = bindings
        .iter()
        .map(|b| {
            serde_json::json!({
                "jsName": b.js_name,
                "name": b.tool_name,
                "server": b.server_name,
                "description": b.description,
                "inputSchema": b.input_schema,
            })
        })
        .collect();
    let list_json = serde_json::Value::Array(list).to_string();
    let list_val = ctx.json_parse(list_json)?;
    ctx.globals().set("__ziee_toollist", list_val)?;

    // Build ziee.call / ziee.tools / ziee.toolList.
    ctx.eval::<(), _>(ZIEE_PRELUDE)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(server: &str, tool: &str) -> RawTool {
        RawTool {
            server_id: Uuid::new_v4(),
            server_name: server.to_string(),
            tool_name: tool.to_string(),
            description: String::new(),
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    // TEST-8: unique names bind directly; collisions get server-prefixed.
    #[test]
    fn test_binding_names_unique_and_collision_safe() {
        let tools = vec![
            raw("web_search", "web_search"),
            raw("memory", "recall"),
            raw("serverA", "search"),
            raw("serverB", "search"), // collides with serverA's `search`
        ];
        let b = build_bindings(&tools);
        let names: Vec<&str> = b.iter().map(|x| x.js_name.as_str()).collect();
        assert!(names.contains(&"web_search"));
        assert!(names.contains(&"recall"));
        // Both `search` tools became server-prefixed and are distinct.
        assert!(names.contains(&"serverA_search"), "names: {names:?}");
        assert!(names.contains(&"serverB_search"), "names: {names:?}");
        // All js_names are unique.
        let uniq: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(uniq.len(), names.len());
    }

    #[test]
    fn test_binding_sanitizes_invalid_identifier_chars() {
        let tools = vec![raw("srv", "get-user.info")];
        let b = build_bindings(&tools);
        assert_eq!(b[0].js_name, "get_user_info");
    }

    // TEST-10: the call budget rejects the (max+1)-th claim.
    #[test]
    fn test_call_budget_caps() {
        let budget = CallBudget::new(2);
        assert!(budget.try_claim());
        assert!(budget.try_claim());
        assert!(!budget.try_claim(), "3rd claim must fail");
        assert_eq!(budget.used(), 2);
    }
}
