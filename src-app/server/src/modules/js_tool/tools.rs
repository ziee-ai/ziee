//! Static `run_js` tool descriptor emitted by `tools/list`.

use serde_json::{Value, json};

/// Wire name of the single tool this built-in exposes.
pub const RUN_JS_TOOL_NAME: &str = "run_js";

pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": RUN_JS_TOOL_NAME,
                "description": concat!(
                    "Run JavaScript in a sandboxed in-process runtime that can call the SAME tools ",
                    "you have, as async host functions, and return only a FINAL value to your context. ",
                    "Use this for programmatic tool calling: loop over many items, filter/aggregate ",
                    "large tool results, or chain tools — the intermediate tool results stay inside the ",
                    "script and never fill your context; only what you `return` comes back.\n\n",
                    "Inside the script:\n",
                    "- `await ziee.tools.<name>({ ...args })` calls a tool (e.g. `await ziee.tools.web_search({ query: 'x' })`).\n",
                    "- `ziee.toolList()` returns the available tools with their input schemas — call it first to discover exact binding names.\n",
                    "- `await ziee.call(name, args)` is the dynamic form.\n",
                    "- `console.log(...)` is captured (bounded) for your inspection.\n",
                    "- The script is an async function body: use `await` and `return <finalValue>`.\n\n",
                    "A tool result is `{ content, structuredContent, isError }`. There is NO filesystem, ",
                    "network, `fetch`, `require`, or `process` — the injected `ziee.*` functions are the ",
                    "only capability. Calls to tools that require approval will pause for the user; a ",
                    "denied call throws `ToolApprovalDenied` you can catch. CPU, memory, wall-clock, and ",
                    "tool-call count are bounded; on error you get the message + line number to retry once."
                ),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "script": {
                            "type": "string",
                            "description": "JavaScript (async function body). Use `await ziee.tools.*` / `ziee.call` and `return` the final value."
                        }
                    },
                    "required": ["script"]
                }
            }
        ]
    })
}
