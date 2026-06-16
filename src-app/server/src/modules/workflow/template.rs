//! Regex-based `{{ … }}` template engine (plan §4.5 — Phase 1
//! baseline; the pest/lalrpop upgrade for the type-inference path
//! lives in `template_parser.rs` in a later phase).
//!
//! Variable grammar (FULL access chain — must stay in lockstep with
//! `ref_check.rs`'s parser so what the validator accepts is exactly what
//! this engine can resolve):
//!   `{{ inputs.<name> }}`             — workflow input value
//!   `{{ inputs.<name>.field }}`       — field access into an input value
//!   `{{ inputs.<name>[<n>] }}`        — array element of an input value
//!   `{{ <step_id>.output }}`          — step output content (lazy
//!                                       file read via `file_io`)
//!   `{{ <step_id>.output[<n>] }}`     — array element
//!   `{{ <step_id>.output.field }}`    — object field access
//!   `{{ <step_id>.output.field[N].sub }}` — arbitrary nesting
//!   `{{ <step_id>.path }}`            — sandbox-visible output path
//!                                       string (`outputs/<step_id>{.json|.txt}`)
//!   `{{ <step_id>.output | json }}`   — force JSON-serialize
//!   `{{ <step_id>.output | raw }}`    — bypass JSON for arrays/objects
//!
//! Default scalar coercion:
//!   `String("hello")` → `hello` (bare, no quotes)
//!   `Number/Bool/Null` → JSON literal (`42`, `true`, `null`)
//!   `Array/Object` → JSON-serialized
//!
//! Unknown variable / missing field / out-of-bounds index / field-on-
//! non-object → the documented `TemplateError` (§4.5).

#![allow(dead_code)]

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use thiserror::Error;

use crate::modules::workflow::types::RunContext;

static VAR_RE: Lazy<Regex> = Lazy::new(|| {
    // `{{ <expr> }}` where <expr> is `name(.field|[N])*( | filter)?`.
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
    #[error("field '{field}' not present on object (path '{path}')")]
    MissingField { field: String, path: String },
    #[error("cannot access field '{field}' on a non-object value (path '{path}')")]
    NotAnObject { field: String, path: String },
    #[error("io error reading step output: {0}")]
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    None,
    Json,
    Raw,
}

/// One access segment after the head, e.g. `.output`, `[2]`, `.title`.
/// Mirrors `ref_check::Access` so the two parsers stay in lockstep.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Access {
    Field(String),
    Index(usize),
}

#[derive(Debug)]
struct ParsedExpr {
    head: String,         // "inputs" or "<step_id>"
    accesses: Vec<Access>, // full chain after the head
    filter: Filter,
}

impl ParsedExpr {
    /// Human-readable rendering of the full reference (for diagnostics).
    fn render(&self) -> String {
        let mut s = self.head.clone();
        for a in &self.accesses {
            match a {
                Access::Field(f) => {
                    s.push('.');
                    s.push_str(f);
                }
                Access::Index(i) => {
                    s.push('[');
                    s.push_str(&i.to_string());
                    s.push(']');
                }
            }
        }
        s
    }
}

/// Parse a `{{ … }}` body into a head + full access chain + optional
/// filter. The access-chain parsing is identical in shape to
/// `ref_check::parse_ref` (Field/Index segments) so the validator and the
/// runtime agree on what's a valid reference.
fn parse_expr(s: &str) -> Result<ParsedExpr, TemplateError> {
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

    if lhs.is_empty() {
        return Err(TemplateError::InvalidSyntax(s.to_string()));
    }

    // Read the head identifier (up to the first `.` or `[`).
    let head_end = lhs
        .char_indices()
        .find(|(_, c)| *c == '.' || *c == '[')
        .map(|(i, _)| i)
        .unwrap_or(lhs.len());
    let head = lhs[..head_end].trim().to_string();
    if head.is_empty() {
        return Err(TemplateError::InvalidSyntax(format!(
            "missing head identifier in '{s}'"
        )));
    }

    let mut accesses = Vec::new();
    let mut rest = &lhs[head_end..];
    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix('.') {
            // `.field` — read until next `.` or `[`.
            let mut end = stripped.len();
            for (i, c) in stripped.char_indices() {
                if c == '.' || c == '[' {
                    end = i;
                    break;
                }
            }
            let field = stripped[..end].trim();
            if field.is_empty() {
                return Err(TemplateError::InvalidSyntax(format!(
                    "empty field access in '{s}'"
                )));
            }
            accesses.push(Access::Field(field.to_string()));
            rest = &stripped[end..];
        } else if let Some(stripped) = rest.strip_prefix('[') {
            // `[N]` — read until `]`.
            let close = stripped
                .find(']')
                .ok_or_else(|| TemplateError::InvalidSyntax(format!("unbalanced [ in '{s}'")))?;
            let idx_str = stripped[..close].trim();
            let idx: usize = idx_str
                .parse()
                .map_err(|_| TemplateError::InvalidSyntax(format!("non-numeric index '{idx_str}'")))?;
            accesses.push(Access::Index(idx));
            rest = &stripped[close + 1..];
        } else {
            return Err(TemplateError::InvalidSyntax(format!(
                "unexpected token near '{rest}' in '{s}'"
            )));
        }
    }

    Ok(ParsedExpr {
        head,
        accesses,
        filter,
    })
}

/// Resolve a single variable expression against the context.
fn resolve_expr(expr: &ParsedExpr, ctx: &RunContext) -> Result<Value, TemplateError> {
    let head = expr.head.as_str();
    match head {
        "inputs" => {
            // First access is the input name.
            let Some(Access::Field(name)) = expr.accesses.first() else {
                return Err(TemplateError::InvalidSyntax(format!(
                    "'inputs' must be followed by an input name (in '{}')",
                    expr.render()
                )));
            };
            let base = ctx
                .inputs
                .get(name)
                .cloned()
                .ok_or_else(|| TemplateError::UnknownVariable(format!("inputs.{name}")))?;
            walk_value(base, &expr.accesses[1..], expr)
        }
        step_id => {
            let meta = ctx
                .step_outputs
                .get(step_id)
                .ok_or_else(|| TemplateError::StepNotYetRun(step_id.to_string()))?;
            let Some(first) = expr.accesses.first() else {
                return Err(TemplateError::UnknownVariable(format!(
                    "{step_id} (expected '.output' or '.path')"
                )));
            };
            match first {
                Access::Field(f) if f == "output" => {
                    // Read the file content lazily — single-step templates
                    // touch the disk only when actually referenced.
                    let base = crate::modules::workflow::file_io::read_output_value(meta)
                        .map_err(|e| TemplateError::Io(e.to_string()))?;
                    walk_value(base, &expr.accesses[1..], expr)
                }
                Access::Field(f) if f == "path" => {
                    // Sandbox-visible path. The runner stages the outputs/
                    // dir RO into the sandbox at CWD; the path string is
                    // "outputs/<step_id>{.json|.txt}". No further access is
                    // valid on a path string.
                    if expr.accesses.len() > 1 {
                        return Err(TemplateError::InvalidSyntax(format!(
                            "'.path' is a string; cannot access '{}' on it",
                            expr.render()
                        )));
                    }
                    let path = ctx.step_output_sandbox_path(step_id);
                    Ok(Value::String(path))
                }
                other => Err(TemplateError::UnknownVariable(format!(
                    "{step_id}.{}",
                    access_label(other)
                ))),
            }
        }
    }
}

/// Walk a chain of `.field` / `[N]` accesses against a starting JSON
/// value. Object field access + array indexing; out-of-bounds / missing
/// field / wrong-type → the documented error per §4.5.
fn walk_value(start: Value, accesses: &[Access], expr: &ParsedExpr) -> Result<Value, TemplateError> {
    let mut cur = start;
    for acc in accesses {
        match acc {
            Access::Index(i) => match cur {
                Value::Array(mut a) => {
                    if *i >= a.len() {
                        return Err(TemplateError::IndexOutOfBounds {
                            idx: *i,
                            len: a.len(),
                        });
                    }
                    cur = a.swap_remove(*i);
                }
                _ => return Err(TemplateError::NotAnArray),
            },
            Access::Field(field) => match cur {
                Value::Object(mut map) => match map.remove(field) {
                    Some(v) => cur = v,
                    None => {
                        return Err(TemplateError::MissingField {
                            field: field.clone(),
                            path: expr.render(),
                        });
                    }
                },
                _ => {
                    return Err(TemplateError::NotAnObject {
                        field: field.clone(),
                        path: expr.render(),
                    });
                }
            },
        }
    }
    Ok(cur)
}

fn access_label(a: &Access) -> String {
    match a {
        Access::Field(f) => f.clone(),
        Access::Index(i) => format!("[{i}]"),
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

/// Render `template` against `ctx`, additionally binding `extra` as
/// resolvable top-level variables (head matched BEFORE `inputs` / step
/// ids). Used by `llm_map` to bind the per-item `item_var → item Value`
/// so `{{ item }}` / `{{ item.field }}` / `{{ item[0] }}` resolve through
/// the same chain-walker (H4). The `extra` head is resolved by walking
/// the bound value directly; if no `extra` entry matches the head we fall
/// back to the normal `inputs` / step resolution.
pub fn render_with_bindings(
    template: &str,
    ctx: &RunContext,
    extra: &HashMap<String, Value>,
) -> Result<String, TemplateError> {
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
        let resolved = if let Some(bound) = extra.get(&parsed.head) {
            // The whole access chain walks the bound value directly (no
            // `.output` indirection — the binding IS the value).
            walk_value(bound.clone(), &parsed.accesses, &parsed)
        } else {
            resolve_expr(&parsed, ctx)
        };
        let resolved = match resolved {
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
/// `RunContext` yet — checks ONLY the syntactic shape and the set of
/// referenced variables. Returns the unique set of `(head, leading_field)`
/// pairs the template references, where `leading_field` is the FIRST
/// access segment after the head (`output` / `path` / the input name) —
/// NOT the whole chain. This keeps `validate.rs::check_template_refs`'s
/// name-level + leading-field check correct for chained references like
/// `{{ confirm.output.proceed }}` (leading field = `output`).
pub fn scan_var_refs(template: &str) -> Result<Vec<(String, String)>, TemplateError> {
    let mut out: Vec<(String, String)> = Vec::new();
    for cap in VAR_RE.captures_iter(template) {
        let body = cap.get(1).unwrap().as_str();
        let p = parse_expr(body)?;
        let leading = match p.accesses.first() {
            Some(Access::Field(f)) => f.clone(),
            // An index or no access after the head: surface the head with
            // an empty leading field so the name check still runs on the
            // head; validate.rs treats non-output/path leading fields as
            // errors only for step heads (inputs handled separately).
            Some(Access::Index(_)) | None => String::new(),
        };
        let pair = (p.head.clone(), leading);
        if !out.contains(&pair) {
            out.push(pair);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::workflow::types::{OutputMeta, ParsedAs, StepKindTag};
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
            model_max_tokens: 8192,
            sandbox_flavor: None,
            total_tokens: 0,
            total_output_bytes: 0,
            is_dev: false,
            mocks: std::collections::HashMap::new(),
            force_mocks: false,
        }
    }

    /// Helper: write a step output file + register its meta on the ctx so
    /// `{{ <step>.output … }}` resolves against real on-disk content.
    fn put_step_output(ctx: &mut RunContext, step_id: &str, value: &Value, parsed_as: ParsedAs) {
        std::fs::create_dir_all(&ctx.outputs_dir).unwrap();
        let ext = match parsed_as {
            ParsedAs::Json => "json",
            ParsedAs::Text => "txt",
        };
        let path = ctx.outputs_dir.join(format!("{step_id}.{ext}"));
        let bytes = match parsed_as {
            ParsedAs::Json => serde_json::to_vec(value).unwrap(),
            ParsedAs::Text => match value {
                Value::String(s) => s.clone().into_bytes(),
                other => serde_json::to_vec(other).unwrap(),
            },
        };
        std::fs::write(&path, &bytes).unwrap();
        ctx.step_outputs.insert(
            step_id.to_string(),
            OutputMeta {
                path,
                size_bytes: bytes.len() as u64,
                sha256: String::new(),
                preview: String::new(),
                kind: StepKindTag::Llm,
                parsed_as,
            },
        );
    }

    fn unique_ctx() -> RunContext {
        // Isolate disk paths per-test so concurrent runs don't collide.
        let mut ctx = fake_ctx();
        let base = std::env::temp_dir().join(format!("ziee-tmpl-{}", uuid::Uuid::new_v4()));
        ctx.outputs_dir = base.join("outputs");
        ctx.sandbox_workspace = base.join("ws");
        ctx
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
    fn scan_var_refs_dedupes_and_returns_leading_field() {
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
    fn scan_var_refs_chain_returns_leading_field_only() {
        // C1: a chained ref's leading field is "output", NOT "output.proceed".
        let refs = scan_var_refs("{{ confirm.output.proceed }}").unwrap();
        assert_eq!(refs, vec![("confirm".to_string(), "output".to_string())]);
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

    // ── C1: full access-chain resolution ──────────────────────────────

    #[test]
    fn step_output_field_resolves() {
        // {{ s.output.field }} walks into the object.
        let mut ctx = unique_ctx();
        put_step_output(
            &mut ctx,
            "gen",
            &serde_json::json!({"title": "Hello", "n": 3}),
            ParsedAs::Json,
        );
        let s = render("t={{ gen.output.title }} n={{ gen.output.n }}", &ctx).unwrap();
        assert_eq!(s, "t=Hello n=3");
    }

    #[test]
    fn step_output_index_resolves() {
        // {{ s.output[0] }} indexes into the array output.
        let mut ctx = unique_ctx();
        put_step_output(
            &mut ctx,
            "fan",
            &serde_json::json!(["alpha", "beta", "gamma"]),
            ParsedAs::Json,
        );
        let s = render("x={{ fan.output[0] }} y={{ fan.output[2] }}", &ctx).unwrap();
        assert_eq!(s, "x=alpha y=gamma");
    }

    #[test]
    fn step_output_nested_chain_resolves() {
        // {{ s.output.field[N].sub }} — arbitrary nesting.
        let mut ctx = unique_ctx();
        put_step_output(
            &mut ctx,
            "research",
            &serde_json::json!({"items": [{"sub": "first"}, {"sub": "second"}]}),
            ParsedAs::Json,
        );
        let s = render("{{ research.output.items[1].sub }}", &ctx).unwrap();
        assert_eq!(s, "second");
    }

    #[test]
    fn elicit_object_readback_resolves() {
        // The elicit object-readback feature: {{ confirm.output.proceed }}.
        let mut ctx = unique_ctx();
        put_step_output(
            &mut ctx,
            "confirm",
            &serde_json::json!({"proceed": true, "max_sources": 7}),
            ParsedAs::Json,
        );
        let s = render(
            "go={{ confirm.output.proceed }} max={{ confirm.output.max_sources }}",
            &ctx,
        )
        .unwrap();
        assert_eq!(s, "go=true max=7");
    }

    #[test]
    fn missing_field_errors() {
        let mut ctx = unique_ctx();
        put_step_output(
            &mut ctx,
            "gen",
            &serde_json::json!({"title": "x"}),
            ParsedAs::Json,
        );
        let err = render("{{ gen.output.nope }}", &ctx).unwrap_err();
        assert!(matches!(err, TemplateError::MissingField { .. }), "got {err:?}");
    }

    #[test]
    fn out_of_bounds_index_errors() {
        let mut ctx = unique_ctx();
        put_step_output(&mut ctx, "fan", &serde_json::json!(["a"]), ParsedAs::Json);
        let err = render("{{ fan.output[5] }}", &ctx).unwrap_err();
        assert!(matches!(err, TemplateError::IndexOutOfBounds { .. }), "got {err:?}");
    }

    #[test]
    fn field_on_scalar_errors() {
        let mut ctx = unique_ctx();
        put_step_output(
            &mut ctx,
            "gen",
            &serde_json::json!("just a string"),
            ParsedAs::Json,
        );
        let err = render("{{ gen.output.title }}", &ctx).unwrap_err();
        assert!(matches!(err, TemplateError::NotAnObject { .. }), "got {err:?}");
    }

    #[test]
    fn for_each_chain_resolves_to_array_json() {
        // for_each: "{{ x.output.items }}" must render the array as JSON so
        // the llm_map dispatcher can parse it back.
        let mut ctx = unique_ctx();
        put_step_output(
            &mut ctx,
            "x",
            &serde_json::json!({"items": ["q1", "q2"]}),
            ParsedAs::Json,
        );
        let s = render("{{ x.output.items }}", &ctx).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&s).unwrap(), serde_json::json!(["q1", "q2"]));
    }

    // ── H4: per-item binding for llm_map ──────────────────────────────

    #[test]
    fn render_with_bindings_item_field() {
        let ctx = fake_ctx();
        let mut extra = HashMap::new();
        extra.insert("q".to_string(), serde_json::json!({"title": "X", "id": 9}));
        let s = render_with_bindings("title={{ q.title }} id={{ q.id }}", &ctx, &extra).unwrap();
        assert_eq!(s, "title=X id=9");
    }

    #[test]
    fn render_with_bindings_item_scalar() {
        let ctx = fake_ctx();
        let mut extra = HashMap::new();
        extra.insert("item".to_string(), serde_json::json!("plain"));
        let s = render_with_bindings("v={{ item }}", &ctx, &extra).unwrap();
        assert_eq!(s, "v=plain");
    }

    #[test]
    fn render_with_bindings_falls_back_to_ctx() {
        // A head that is NOT a binding resolves normally (inputs).
        let ctx = fake_ctx();
        let mut extra = HashMap::new();
        extra.insert("q".to_string(), serde_json::json!("X"));
        let s = render_with_bindings("{{ inputs.topic }}/{{ q }}", &ctx, &extra).unwrap();
        assert_eq!(s, "LLMs/X");
    }

    // ── C1 parity: every ref_check-accepted shape resolves here ────────

    #[test]
    fn c1_parity_ref_check_accepts_template_resolves() {
        // Representative references that ref_check.rs blesses as valid must
        // ALL resolve here against a populated RunContext (no UnknownVariable
        // / syntax error). This is the parity gate: validator-accepted ⊆
        // template-resolvable.
        let mut ctx = unique_ctx();
        ctx.inputs
            .insert("qs".to_string(), serde_json::json!(["a", "b"]));
        put_step_output(&mut ctx, "gen", &serde_json::json!("text"), ParsedAs::Text);
        put_step_output(
            &mut ctx,
            "confirm",
            &serde_json::json!({"proceed": true}),
            ParsedAs::Json,
        );
        put_step_output(
            &mut ctx,
            "fan",
            &serde_json::json!(["x0", "x1"]),
            ParsedAs::Json,
        );

        let accepted = [
            "{{ inputs.qs }}",
            "{{ inputs.qs[0] }}",
            "{{ gen.output }}",
            "{{ gen.path }}",
            "{{ confirm.output.proceed }}",
            "{{ fan.output[0] }}",
        ];
        for tpl in accepted {
            let r = render(tpl, &ctx);
            assert!(r.is_ok(), "template '{tpl}' failed to resolve: {:?}", r.err());
        }
    }
}
