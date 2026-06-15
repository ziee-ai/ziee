//! Regex-based `{{ … }}` template engine (plan §4.5 — Phase 1
//! baseline; the pest/lalrpop upgrade for the type-inference path
//! lives in `template_parser.rs` in a later phase).
//!
//! Variable grammar:
//!   `{{ inputs.<name> }}`            — workflow input value
//!   `{{ <step_id>.output }}`         — step output content (lazy
//!                                      file read via `file_io`)
//!   `{{ <step_id>.output[<n>] }}`    — array element (one level)
//!   `{{ <step_id>.path }}`           — sandbox-visible output path
//!                                      string (`outputs/<step_id>{.json|.txt}`
//!                                      or `artifacts/<step_id>/<filename>`
//!                                      — runner picks)
//!   `{{ <step_id>.output | json }}`  — force JSON-serialize
//!   `{{ <step_id>.output | raw }}`   — bypass JSON for arrays/objects
//!
//! Default scalar coercion:
//!   `String("hello")` → `hello` (bare, no quotes)
//!   `Number/Bool/Null` → JSON literal (`42`, `true`, `null`)
//!   `Array/Object` → JSON-serialized
//!
//! Unknown variable → `TemplateError::UnknownVariable`.

#![allow(dead_code)]

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use thiserror::Error;

use crate::modules::workflow::types::RunContext;

static VAR_RE: Lazy<Regex> = Lazy::new(|| {
    // `{{ <expr> }}` where <expr> is `name(.field|.field[N])*( | filter)?`.
    // Whitespace inside braces is allowed and trimmed.
    Regex::new(r"\{\{\s*([^}]+?)\s*\}\}").expect("template regex")
});

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("unknown variable: {0}")]
    UnknownVariable(String),
    #[error("invalid template syntax: {0}")]
    InvalidSyntax(String),
    #[error("template references step '{0}' which has not produced output yet")]
    StepNotYetRun(String),
    #[error("unknown filter '{0}' (supported: json, raw)")]
    UnknownFilter(String),
    #[error("array index {idx} out of bounds (len = {len})")]
    IndexOutOfBounds { idx: usize, len: usize },
    #[error("cannot index non-array value")]
    NotAnArray,
    #[error("io error reading step output: {0}")]
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    None,
    Json,
    Raw,
}

#[derive(Debug)]
struct ParsedExpr<'a> {
    head: &'a str,                // "inputs" or "<step_id>"
    field: &'a str,               // "output" / "path" / "<input_name>"
    index: Option<usize>,         // [N] one level only
    filter: Filter,
}

fn parse_expr(s: &str) -> Result<ParsedExpr<'_>, TemplateError> {
    // Split on the `|` for the optional filter.
    let (lhs, filter) = match s.find('|') {
        Some(i) => {
            let f = s[i + 1..].trim();
            let filter = match f {
                "json" => Filter::Json,
                "raw" => Filter::Raw,
                other => return Err(TemplateError::UnknownFilter(other.to_string())),
            };
            (s[..i].trim(), filter)
        }
        None => (s.trim(), Filter::None),
    };

    // Parse head and field.
    let mut parts = lhs.splitn(2, '.');
    let head = parts
        .next()
        .ok_or_else(|| TemplateError::InvalidSyntax(s.to_string()))?
        .trim();
    let rest = parts
        .next()
        .ok_or_else(|| TemplateError::InvalidSyntax(format!("missing .field in '{s}'")))?
        .trim();

    // Optional `[N]` index on the field.
    let (field, index) = if let Some(open) = rest.find('[') {
        let close = rest
            .rfind(']')
            .ok_or_else(|| TemplateError::InvalidSyntax(format!("unbalanced [ in '{s}'")))?;
        if close < open {
            return Err(TemplateError::InvalidSyntax(format!(
                "unbalanced index in '{s}'"
            )));
        }
        let field = rest[..open].trim();
        let idx_str = rest[open + 1..close].trim();
        let idx: usize = idx_str
            .parse()
            .map_err(|_| TemplateError::InvalidSyntax(format!("non-numeric index '{idx_str}'")))?;
        (field, Some(idx))
    } else {
        (rest, None)
    };
    Ok(ParsedExpr {
        head,
        field,
        index,
        filter,
    })
}

/// Resolve a single variable expression against the context.
fn resolve_expr(expr: &ParsedExpr, ctx: &RunContext) -> Result<Value, TemplateError> {
    let head = expr.head;
    match head {
        "inputs" => {
            let v = ctx
                .inputs
                .get(expr.field)
                .cloned()
                .ok_or_else(|| TemplateError::UnknownVariable(format!("inputs.{}", expr.field)))?;
            apply_index(v, expr.index)
        }
        step_id => {
            let meta = ctx
                .step_outputs
                .get(step_id)
                .ok_or_else(|| TemplateError::StepNotYetRun(step_id.to_string()))?;
            match expr.field {
                "output" => {
                    // Read the file content lazily — single-step
                    // templates touch the disk only when actually
                    // referenced.
                    let v = crate::modules::workflow::file_io::read_output_value(meta)
                        .map_err(|e| TemplateError::Io(e.to_string()))?;
                    apply_index(v, expr.index)
                }
                "path" => {
                    // Sandbox-visible path. The runner stages the
                    // outputs/ dir RO into the sandbox at CWD; the
                    // path string is "outputs/<step_id>{.json|.txt}".
                    if expr.index.is_some() {
                        return Err(TemplateError::InvalidSyntax(
                            "[N] not valid on .path".to_string(),
                        ));
                    }
                    let path = ctx.step_output_sandbox_path(step_id);
                    Ok(Value::String(path))
                }
                other => Err(TemplateError::UnknownVariable(format!(
                    "{step_id}.{other}"
                ))),
            }
        }
    }
}

fn apply_index(v: Value, idx: Option<usize>) -> Result<Value, TemplateError> {
    match idx {
        None => Ok(v),
        Some(i) => match v {
            Value::Array(mut a) => {
                if i >= a.len() {
                    Err(TemplateError::IndexOutOfBounds { idx: i, len: a.len() })
                } else {
                    Ok(a.swap_remove(i))
                }
            }
            _ => Err(TemplateError::NotAnArray),
        },
    }
}

fn stringify(v: &Value, filter: Filter) -> String {
    match filter {
        Filter::Json => serde_json::to_string(v).unwrap_or_else(|_| "null".into()),
        Filter::Raw => match v {
            Value::String(s) => s.clone(),
            _ => serde_json::to_string(v).unwrap_or_else(|_| "null".into()),
        },
        Filter::None => match v {
            Value::String(s) => s.clone(),
            // Number/Bool/Null → JSON literal; Array/Object → JSON-serialized.
            other => serde_json::to_string(other).unwrap_or_else(|_| "null".into()),
        },
    }
}

/// Top-level: render `template` against `ctx`. Returns the rendered
/// string with every `{{ … }}` substituted, or the first error.
pub fn render(template: &str, ctx: &RunContext) -> Result<String, TemplateError> {
    let mut out = String::with_capacity(template.len());
    let mut last = 0usize;
    let mut error: Option<TemplateError> = None;
    for cap in VAR_RE.captures_iter(template) {
        let m = cap.get(0).unwrap();
        out.push_str(&template[last..m.start()]);
        last = m.end();
        let body = cap.get(1).unwrap().as_str();
        let parsed = match parse_expr(body) {
            Ok(p) => p,
            Err(e) => {
                error = Some(e);
                break;
            }
        };
        let resolved = match resolve_expr(&parsed, ctx) {
            Ok(v) => v,
            Err(e) => {
                error = Some(e);
                break;
            }
        };
        out.push_str(&stringify(&resolved, parsed.filter));
    }
    if let Some(e) = error {
        return Err(e);
    }
    out.push_str(&template[last..]);
    Ok(out)
}

/// Compatibility shim used by validators that don't have a full
/// `RunContext` yet — checks ONLY the syntactic shape and the set
/// of referenced variables. Returns the unique set of `(head, field)`
/// pairs the template references, or a syntax error.
pub fn scan_var_refs(template: &str) -> Result<Vec<(String, String)>, TemplateError> {
    let mut out: Vec<(String, String)> = Vec::new();
    for cap in VAR_RE.captures_iter(template) {
        let body = cap.get(1).unwrap().as_str();
        let p = parse_expr(body)?;
        let pair = (p.head.to_string(), p.field.to_string());
        if !out.contains(&pair) {
            out.push(pair);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fake_ctx() -> RunContext {
        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), Value::String("LLMs".to_string()));
        inputs.insert("limit".to_string(), Value::Number(serde_json::Number::from(5)));
        RunContext {
            run_id: uuid::Uuid::nil(),
            user_id: uuid::Uuid::nil(),
            conversation_id: None,
            workflow_id: uuid::Uuid::nil(),
            inputs,
            step_outputs: HashMap::new(),
            step_item_progress: HashMap::new(),
            extracted_path: PathBuf::from("/tmp/_"),
            sandbox_workspace: PathBuf::from("/tmp/_/ws"),
            outputs_dir: PathBuf::from("/tmp/_/ws/outputs"),
            artifacts_dir: PathBuf::from("/tmp/_/ws/artifacts"),
            inputs_dir: PathBuf::from("/tmp/_/ws/inputs"),
            model_id: uuid::Uuid::nil(),
            model_name: "test-model".into(),
            sandbox_flavor: None,
            total_tokens: 0,
            total_output_bytes: 0,
            is_dev: false,
            mocks: std::collections::HashMap::new(),
            force_mocks: false,
        }
    }

    #[test]
    fn renders_input_substitution_bare() {
        let s = render("hello {{ inputs.topic }}!", &fake_ctx()).unwrap();
        assert_eq!(s, "hello LLMs!");
    }

    #[test]
    fn renders_number_literal() {
        let s = render("n={{ inputs.limit }}", &fake_ctx()).unwrap();
        assert_eq!(s, "n=5");
    }

    #[test]
    fn json_filter_quotes_strings() {
        let s = render("v={{ inputs.topic | json }}", &fake_ctx()).unwrap();
        assert_eq!(s, "v=\"LLMs\"");
    }

    #[test]
    fn raw_filter_bare_string() {
        let s = render("v={{ inputs.topic | raw }}", &fake_ctx()).unwrap();
        assert_eq!(s, "v=LLMs");
    }

    #[test]
    fn unknown_input_errors() {
        let err = render("{{ inputs.nope }}", &fake_ctx()).unwrap_err();
        assert!(matches!(err, TemplateError::UnknownVariable(_)));
    }

    #[test]
    fn unknown_filter_errors() {
        let err = render("{{ inputs.topic | foo }}", &fake_ctx()).unwrap_err();
        assert!(matches!(err, TemplateError::UnknownFilter(_)));
    }

    #[test]
    fn step_path_returns_sandbox_relative() {
        use crate::modules::workflow::types::{OutputMeta, ParsedAs, StepKindTag};
        let mut ctx = fake_ctx();
        ctx.step_outputs.insert(
            "build".into(),
            OutputMeta {
                path: PathBuf::from("/tmp/_/ws/outputs/build.txt"),
                size_bytes: 0,
                sha256: String::new(),
                preview: String::new(),
                kind: StepKindTag::Sandbox,
                parsed_as: ParsedAs::Text,
            },
        );
        let s = render("p={{ build.path }}", &ctx).unwrap();
        assert_eq!(s, "p=outputs/build.txt");
    }

    #[test]
    fn scan_var_refs_dedupes() {
        let refs = scan_var_refs("{{ inputs.a }} and {{ inputs.a }} and {{ s1.output }}")
            .unwrap();
        assert_eq!(
            refs,
            vec![
                ("inputs".to_string(), "a".to_string()),
                ("s1".to_string(), "output".to_string()),
            ]
        );
    }

    #[test]
    fn index_on_input_array() {
        let mut ctx = fake_ctx();
        ctx.inputs.insert(
            "items".to_string(),
            serde_json::json!(["a", "b", "c"]),
        );
        let s = render("first={{ inputs.items[1] }}", &ctx).unwrap();
        assert_eq!(s, "first=b");
    }
}
