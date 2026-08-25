//! Turn an OpenAPI request schema into a **self-contained** JSON Schema the
//! chat model can actually read.
//!
//! `Operation::request_schema` is copied verbatim out of the OpenAPI document,
//! where schemars always emits a named request body as
//! `{"$ref": "#/components/schemas/CreateProjectRequest"}`. The model has no
//! access to `#/components/*` — that document lives in this process — so a bare
//! `$ref` tells it nothing and it must guess the body. This module resolves
//! those pointers so `describe_capability` returns the real field contract.
//!
//! Three properties matter, in this order:
//!
//! 1. **Self-contained.** No `#/components/…` pointer survives, on ANY path.
//!    Where a reference cannot be expanded (see below) it is rewritten to
//!    `#/$defs/<Name>` and the named schema is emitted into a root `$defs`,
//!    closed to a fixpoint. `$defs` + `#/$defs/…` is standard JSON Schema
//!    2020-12, so the result is still resolvable with no external document.
//! 2. **Terminating.** Recursion is detected by an explicit resolution STACK,
//!    not a depth heuristic: a `$ref` re-entering a schema already being
//!    expanded is cut to a `$defs` self-reference — which is precisely how a
//!    recursive schema is written — so a self-referential or mutually recursive
//!    component can neither loop nor blow the stack.
//! 3. **Bounded.** The output goes into the model's context window, so ref
//!    EXPANSION (the only thing that can multiply size) is budgeted, and
//!    over-budget input degrades in ordered, still-valid steps rather than being
//!    truncated into broken JSON. See [`Budget`].
//!
//! Note the budget bounds *expansion*, not the operation's own declared schema:
//! a ref-free root is passed through untouched however large it is, because
//! nothing was expanded and mutilating a schema we were handed would be a
//! different kind of lie than the one this module exists to fix.
//!
//! Traversal mirrors `ziee_control_mcp::catalog`'s `schema_has_secret_field_rec`
//! (resolve, then descend through `properties` / `items` / `allOf`+`oneOf`+
//! `anyOf`) and deliberately diverges on one point: that function stops at a
//! blind `depth > 6`, which is correct for a boolean "is a secret in here" probe
//! and wrong for a schema we EMIT — a silent truncation there would reproduce
//! the very defect this module fixes.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

/// Prefix of a local component pointer in an OpenAPI document.
const COMPONENT_PREFIX: &str = "#/components/schemas/";
/// Prefix of a local pointer in the SELF-CONTAINED document we emit.
const DEFS_PREFIX: &str = "#/$defs/";

/// Bounds on ref expansion.
///
/// Grouped in one struct (rather than inline magic numbers) so the values are
/// named, documented, and promotable without a rewrite. They are deliberately
/// NOT admin-configurable: they bound one request-scoped transformation of a
/// document the operator does not author, and the only thing a different value
/// changes is how much of a schema the model sees — there is no deployment for
/// which another value is right, and a wrong one either starves the model or
/// floods its context. The module's operational switch is `control_mcp.enabled`.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Maximum nesting depth to expand through. A deeper reference is deferred
    /// to `$defs` rather than expanded.
    pub max_depth: usize,
    /// Maximum number of `$ref`s expanded in one schema. Bounds BREADTH, which
    /// is what actually multiplies size (the widest real operation expands 14).
    pub max_expansions: usize,
    /// Soft cap: past this, the whole schema is re-emitted in the compact
    /// `$defs` form, where each referenced schema appears exactly once.
    pub max_bytes: usize,
    /// Hard cap: past this, the largest `$defs` entries are elided to named
    /// placeholders. Never a mid-structure cut.
    pub hard_max_bytes: usize,
}

/// Measured against the committed `src-app/ui/openapi/openapi.json`: of the 140
/// operations with a JSON body, the largest fully-inlined schema is
/// `LlmModel.create` at 10,522 bytes / 11 expansions, the widest is
/// `Workflow.create` at 14 expansions, the median is 349 bytes, and none is
/// cyclic. So `max_bytes` sits ~6x above the worst real case: on today's spec
/// every operation takes the plain inline path and the degradation machinery is
/// a guard against a pathological future schema, not a routine code path.
pub const DEFAULT_BUDGET: Budget = Budget {
    max_depth: 12,
    max_expansions: 200,
    max_bytes: 64 * 1024,
    hard_max_bytes: 256 * 1024,
};

/// Which shape the emitted schema took. Reported to the model so a `$defs`
/// document is never mistaken for an unresolved one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaForm {
    /// Every reference was expanded in place.
    Inline,
    /// Some (or all) references are expressed as `$defs` entries — because of a
    /// cycle, the depth/expansion budget, or the size budget.
    Defs,
}

impl SchemaForm {
    pub fn as_str(self) -> &'static str {
        match self {
            SchemaForm::Inline => "inline",
            SchemaForm::Defs => "defs",
        }
    }
}

/// The result of inlining: a self-contained schema plus what had to be done to
/// keep it bounded.
#[derive(Debug, Clone)]
pub struct InlinedSchema {
    pub schema: Value,
    pub form: SchemaForm,
    /// True only when a `$defs` entry was ELIDED for size — i.e. some type
    /// information is genuinely absent. A `$defs` reference is NOT truncation.
    pub truncated: bool,
}

/// Inline `schema` against the OpenAPI `components` object with [`DEFAULT_BUDGET`].
pub fn inline_schema(schema: &Value, components: &Value) -> InlinedSchema {
    inline_schema_with(schema, components, &DEFAULT_BUDGET)
}

/// Inline `schema` against `components` under an explicit budget (the seam the
/// unit tests drive, so the degradation paths are provable without fabricating
/// a 64 KiB fixture).
pub fn inline_schema_with(schema: &Value, components: &Value, budget: &Budget) -> InlinedSchema {
    let mut w = Walker {
        components,
        budget,
        stack: Vec::new(),
        deferred: BTreeSet::new(),
        expansions: 0,
    };
    let mut root = w.walk(schema, 0);
    let expanded_any = w.expansions > 0;
    let defs = close_defs(components, w.deferred);

    // Derived from what was actually DEFERRED, not from "the root has a `$defs`
    // key": a source schema may carry its own `$defs` with nothing deferred, and
    // reporting that as `defs` would tell the model shared/recursive types live
    // there when none do.
    let form = if defs.is_empty() {
        SchemaForm::Inline
    } else {
        SchemaForm::Defs
    };
    if !defs.is_empty() {
        attach_defs(&mut root, defs);
    }

    if serialized_len(&root) <= budget.max_bytes || !expanded_any {
        // `!expanded_any`: a ref-free schema is the operation's own declared
        // shape. Nothing was expanded, so there is nothing for the compact form
        // to compact — falling through would relabel it `defs` and emit an empty
        // `$defs`. The budget bounds EXPANSION, not what we were handed.
        return InlinedSchema {
            schema: root,
            form,
            truncated: false,
        };
    }

    // Soft cap exceeded: re-emit in the compact form, where each referenced
    // schema appears exactly ONCE instead of at every use site.
    compact_form(schema, components, budget)
}

/// Follow a single top-level `#/components/schemas/Name`. Returns the input
/// unchanged when it is not such a `$ref`, or when the name is unknown — i.e. it
/// FAILS OPEN, which `validate_body` relies on (an unresolvable body schema
/// skips local validation and is left to the real route).
///
/// This is the single app-side ref resolver; `handlers.rs` used to carry a
/// byte-identical private copy.
pub fn resolve_schema_ref(schema: &Value, components: &Value) -> Value {
    let Some(name) = component_ref_name(schema) else {
        return schema.clone();
    };
    component(components, name)
        .cloned()
        .unwrap_or_else(|| schema.clone())
}

// ── the walker ───────────────────────────────────────────────────────────────

struct Walker<'a> {
    components: &'a Value,
    budget: &'a Budget,
    /// Component names currently being expanded — the cycle detector.
    stack: Vec<String>,
    /// Names cut out of the inline expansion; emitted into `$defs`.
    deferred: BTreeSet<String>,
    expansions: usize,
}

impl Walker<'_> {
    fn walk(&mut self, node: &Value, depth: usize) -> Value {
        let Some(obj) = node.as_object() else {
            // `true` / `false` are valid schemas; anything else here is a
            // non-schema literal. Either way there is nothing to resolve.
            return node.clone();
        };

        if obj.contains_key("$ref") {
            return self.walk_ref(obj, depth);
        }

        let mut out = Map::with_capacity(obj.len());
        for (key, value) in obj {
            out.insert(key.clone(), self.walk_keyword(key, value, depth));
        }
        Value::Object(out)
    }

    /// Dispatch on the JSON Schema keyword: which keywords hold subschemas, and
    /// in what shape (single / map-of / array-of).
    fn walk_keyword(&mut self, key: &str, value: &Value, depth: usize) -> Value {
        match key {
            // Map of name → subschema.
            "properties" | "patternProperties" | "$defs" | "definitions" | "dependentSchemas" => {
                match value.as_object() {
                    Some(m) => Value::Object(
                        m.iter()
                            .map(|(k, v)| (k.clone(), self.walk(v, depth + 1)))
                            .collect(),
                    ),
                    None => value.clone(),
                }
            }
            // Array of subschemas.
            "allOf" | "anyOf" | "oneOf" | "prefixItems" => match value.as_array() {
                Some(a) => Value::Array(a.iter().map(|v| self.walk(v, depth + 1)).collect()),
                None => value.clone(),
            },
            // A single subschema — except `items`, which may also be the
            // OpenAPI 3.0 / draft-07 tuple ARRAY form.
            "items" | "additionalItems" | "unevaluatedItems" | "contains" | "not" | "if"
            | "then" | "else" | "propertyNames" => match value {
                Value::Array(a) => Value::Array(a.iter().map(|v| self.walk(v, depth + 1)).collect()),
                Value::Object(_) => self.walk(value, depth + 1),
                other => other.clone(),
            },
            // Boolean OR subschema.
            "additionalProperties" | "unevaluatedProperties" => match value {
                Value::Object(_) => self.walk(value, depth + 1),
                other => other.clone(),
            },
            // Not a subschema keyword (`type`, `required`, `enum`, `default`, …).
            _ => value.clone(),
        }
    }

    fn walk_ref(&mut self, obj: &Map<String, Value>, depth: usize) -> Value {
        let raw = obj.get("$ref").and_then(|r| r.as_str()).unwrap_or_default();
        let Some(name) = raw.strip_prefix(COMPONENT_PREFIX) else {
            // Not a local component pointer (an external URL, a `#/$defs/…`
            // already in the source). Leave the POINTER exactly as we found it —
            // it is not ours to rewrite — but still walk everything beside it,
            // which may carry component refs of its own.
            let mut out = Map::new();
            for (k, v) in obj {
                if k == "$ref" {
                    out.insert(k.clone(), v.clone());
                } else {
                    out.insert(k.clone(), self.walk_keyword(k, v, depth));
                }
            }
            return Value::Object(out);
        };

        if component(self.components, name).is_none() {
            // Dangling: say so, and constrain nothing. `{"$comment": …}` is a
            // valid schema that accepts anything — the honest representation of
            // "the type is unknown", as opposed to inventing constraints or
            // leaving a pointer the reader cannot follow.
            return json!({ "$comment": format!("unresolved $ref: {raw}") });
        }

        let must_defer = self.stack.iter().any(|n| n == name)
            || depth >= self.budget.max_depth
            || self.expansions >= self.budget.max_expansions;
        if must_defer {
            self.deferred.insert(name.to_string());
            // Keep the per-use annotations here too — whether a field keeps its
            // `description` must not depend on which side of the budget it fell.
            let mut out = Map::new();
            out.insert("$ref".into(), json!(format!("{DEFS_PREFIX}{name}")));
            for (k, v) in obj {
                if k != "$ref" {
                    out.insert(k.clone(), self.walk_keyword(k, v, depth));
                }
            }
            return Value::Object(out);
        }

        self.expansions += 1;
        self.stack.push(name.to_string());
        let target = component(self.components, name).cloned().unwrap_or(Value::Null);
        let mut expanded = self.walk(&target, depth + 1);
        self.stack.pop();

        // Keep annotations that sat ALONGSIDE the `$ref` (a per-use
        // `description`/`title`/`default`), which OpenAPI allows and which carry
        // exactly the "what is this field for" text the model needs. The
        // referenced schema's own keys win on conflict.
        if obj.len() > 1 {
            // Walk each sibling rather than cloning it: a sibling is legal
            // JSON Schema and may itself be subschema-bearing, in which case a
            // verbatim copy would smuggle a `#/components/…` pointer through.
            let walked: Vec<(String, Value)> = obj
                .iter()
                .filter(|(k, _)| k.as_str() != "$ref")
                .map(|(k, v)| (k.clone(), self.walk_keyword(k, v, depth)))
                .collect();
            if let Some(target_obj) = expanded.as_object_mut() {
                for (k, v) in walked {
                    target_obj.entry(k).or_insert(v);
                }
            }
        }
        expanded
    }
}

// ── `$defs` closure ──────────────────────────────────────────────────────────

/// Expand `seed` to the full set of component schemas reachable from it, each
/// emitted ONCE with its own refs rewritten into `#/$defs/…`. Terminates because
/// a name is processed at most once.
fn close_defs(components: &Value, seed: BTreeSet<String>) -> Map<String, Value> {
    let mut defs: Map<String, Value> = Map::new();
    let mut queue: Vec<String> = seed.into_iter().collect();

    while let Some(name) = queue.pop() {
        if defs.contains_key(&name) {
            continue;
        }
        let body = match component(components, &name) {
            Some(v) => {
                let mut found = BTreeSet::new();
                let rewritten = rewrite_refs_to_defs(v, &mut found);
                for n in found {
                    if !defs.contains_key(&n) {
                        queue.push(n);
                    }
                }
                rewritten
            }
            None => json!({ "$comment": format!("unresolved $ref: {COMPONENT_PREFIX}{name}") }),
        };
        defs.insert(name, body);
    }
    defs
}

/// Rewrite every `#/components/schemas/X` pointer to `#/$defs/X` WITHOUT
/// expanding anything, recording each name seen. This is what makes a `$defs`
/// entry self-contained: it may reference its siblings, never the components
/// document.
fn rewrite_refs_to_defs(node: &Value, found: &mut BTreeSet<String>) -> Value {
    match node {
        Value::Object(obj) => {
            if let Some(name) = component_ref_name(node) {
                found.insert(name.to_string());
                let mut out = Map::new();
                out.insert("$ref".into(), json!(format!("{DEFS_PREFIX}{name}")));
                for (k, v) in obj {
                    if k != "$ref" {
                        out.insert(k.clone(), v.clone());
                    }
                }
                return Value::Object(out);
            }
            Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), rewrite_refs_to_defs(v, found)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(|v| rewrite_refs_to_defs(v, found)).collect()),
        other => other.clone(),
    }
}

/// Merge a `$defs` map onto the root. The root is always an object here (a cut
/// root is `{"$ref": "#/$defs/X"}`), but a non-object root is wrapped rather
/// than dropped.
fn attach_defs(root: &mut Value, defs: Map<String, Value>) {
    match root.as_object_mut() {
        Some(obj) => {
            let existing = obj
                .entry("$defs")
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(m) = existing.as_object_mut() {
                for (k, v) in defs {
                    m.entry(k).or_insert(v);
                }
            }
        }
        None => {
            let mut obj = Map::new();
            obj.insert("$comment".into(), json!("non-object root schema"));
            obj.insert("$defs".into(), Value::Object(defs));
            *root = Value::Object(obj);
        }
    }
}

// ── size degradation ─────────────────────────────────────────────────────────

/// The compact form: nothing is expanded in place; the root and every reachable
/// component are emitted once, cross-referenced through `$defs`. This is the
/// smallest self-contained representation, so it is what the soft cap falls back
/// to. Past the HARD cap the largest entries are elided to named placeholders —
/// deterministic (size desc, then name) and always whole entries, never a cut
/// inside a structure.
fn compact_form(schema: &Value, components: &Value, budget: &Budget) -> InlinedSchema {
    let mut seed = BTreeSet::new();
    let mut root = rewrite_refs_to_defs(schema, &mut seed);
    let mut defs = close_defs(components, seed);

    let mut truncated = false;
    if !defs.is_empty() {
        let mut sizes: Vec<(usize, String)> = defs
            .iter()
            .map(|(k, v)| (serialized_len(v), k.clone()))
            .collect();
        // Largest first; ties broken by name so the elision set is stable.
        sizes.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        for (size, name) in sizes {
            // Re-measure the ACTUAL document each round rather than tracking an
            // estimate: the estimate ignored the `"Name":` key + comma overhead,
            // and once the loop reached entries smaller than a placeholder it
            // would have GROWN the document while believing it was shrinking it.
            let mut probe = root.clone();
            attach_defs(&mut probe, defs.clone());
            if serialized_len(&probe) <= budget.hard_max_bytes {
                break;
            }
            let placeholder = json!({
                "$comment": format!("schema \"{name}\" omitted for size"),
            });
            // Eliding an entry SMALLER than its own placeholder cannot help; the
            // list is size-descending, so nothing after it can either.
            if size <= serialized_len(&placeholder) {
                break;
            }
            defs.insert(name.clone(), placeholder);
            truncated = true;
        }
    }

    if !defs.is_empty() {
        attach_defs(&mut root, defs);
    }
    InlinedSchema {
        schema: root,
        form: SchemaForm::Defs,
        truncated,
    }
}

// ── small helpers ────────────────────────────────────────────────────────────

/// The component name of a `{"$ref": "#/components/schemas/Name"}` node.
fn component_ref_name(schema: &Value) -> Option<&str> {
    schema
        .get("$ref")
        .and_then(|r| r.as_str())
        .and_then(|r| r.strip_prefix(COMPONENT_PREFIX))
}

fn component<'a>(components: &'a Value, name: &str) -> Option<&'a Value> {
    components.get("schemas").and_then(|s| s.get(name))
}

/// Serialized byte length, or 0 on the (practically unreachable) serialization
/// failure. 0 — not `usize::MAX` — so a failure degrades toward "leave it
/// alone" rather than toward "elide everything": `usize::MAX` would both force
/// the compact path and, via `saturating_sub`, break the elision accounting.
fn serialized_len(v: &Value) -> usize {
    match serde_json::to_string(v) {
        Ok(s) => s.len(),
        Err(e) => {
            tracing::warn!(error = %e, "control_mcp: schema serialization failed while measuring");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn components() -> Value {
        json!({ "schemas": {
            "CreateProjectRequest": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string", "description": "Project name" },
                    "settings": { "$ref": "#/components/schemas/ProjectSettings" }
                }
            },
            "ProjectSettings": {
                "type": "object",
                "properties": {
                    "loop_limit": { "type": "integer", "default": 10 },
                    "servers": {
                        "type": "array",
                        "items": { "$ref": "#/components/schemas/ServerRef" }
                    }
                }
            },
            "ServerRef": {
                "type": "object",
                "properties": { "server_id": { "type": "string" } }
            },
            "Node": {
                "type": "object",
                "properties": {
                    "label": { "type": "string" },
                    "children": {
                        "type": "array",
                        "items": { "$ref": "#/components/schemas/Node" }
                    }
                }
            },
            "A": { "type": "object", "properties": { "b": { "$ref": "#/components/schemas/B" } } },
            "B": { "type": "object", "properties": { "a": { "$ref": "#/components/schemas/A" } } },
            "Extra": { "type": "object", "properties": { "flag": { "type": "boolean" } } }
        }})
    }

    fn text(v: &Value) -> String {
        serde_json::to_string(v).unwrap()
    }

    /// TEST-1 — a top-level `$ref` is replaced by the target's body.
    #[test]
    fn inlines_a_flat_top_level_ref() {
        let out = inline_schema(
            &json!({ "$ref": "#/components/schemas/ServerRef" }),
            &components(),
        );
        assert_eq!(out.form, SchemaForm::Inline);
        assert!(out.schema.get("$ref").is_none(), "the $ref must be gone");
        assert_eq!(out.schema["type"], "object");
        assert!(out.schema["properties"].get("server_id").is_some());
    }

    /// TEST-2 — a `$ref` reached only THROUGH `properties` is inlined, so a
    /// nested object's inner field names survive into the output.
    #[test]
    fn inlines_a_ref_nested_in_properties() {
        let out = inline_schema(
            &json!({ "$ref": "#/components/schemas/CreateProjectRequest" }),
            &components(),
        );
        let settings = &out.schema["properties"]["settings"];
        assert!(settings.get("$ref").is_none(), "nested ref must be expanded");
        assert_eq!(settings["properties"]["loop_limit"]["default"], 10);
    }

    /// TEST-3 — refs inside every single/map/array container keyword.
    #[test]
    fn inlines_refs_in_items_additional_pattern_prefix_and_not() {
        let c = components();
        let src = json!({
            "type": "object",
            "properties": {
                "list": { "type": "array", "items": { "$ref": "#/components/schemas/ServerRef" } },
                "map": { "type": "object", "additionalProperties": { "$ref": "#/components/schemas/ServerRef" } },
                "pat": { "type": "object", "patternProperties": { "^x": { "$ref": "#/components/schemas/ServerRef" } } },
                "tuple": { "type": "array", "prefixItems": [ { "$ref": "#/components/schemas/ServerRef" } ] },
                "banned": { "not": { "$ref": "#/components/schemas/ServerRef" } }
            }
        });
        let out = inline_schema(&src, &c);
        let s = text(&out.schema);
        assert!(!s.contains(COMPONENT_PREFIX), "no component pointer may survive: {s}");
        // Each container expanded to the target's property.
        assert_eq!(out.schema["properties"]["list"]["items"]["properties"]["server_id"]["type"], "string");
        assert_eq!(out.schema["properties"]["map"]["additionalProperties"]["properties"]["server_id"]["type"], "string");
        assert_eq!(out.schema["properties"]["pat"]["patternProperties"]["^x"]["properties"]["server_id"]["type"], "string");
        assert_eq!(out.schema["properties"]["tuple"]["prefixItems"][0]["properties"]["server_id"]["type"], "string");
        assert_eq!(out.schema["properties"]["banned"]["not"]["properties"]["server_id"]["type"], "string");
        // A boolean `additionalProperties` is left alone.
        let b = inline_schema(&json!({ "type": "object", "additionalProperties": false }), &c);
        assert_eq!(b.schema["additionalProperties"], json!(false));
    }

    /// TEST-4 — composition keywords, including the real nullable shape
    /// `anyOf: [{$ref}, {"type":"null"}]` that ziee emits for an optional
    /// sub-object.
    #[test]
    fn inlines_refs_in_all_any_one_of() {
        let c = components();
        let src = json!({
            "type": "object",
            "properties": {
                "nullable": { "anyOf": [ { "$ref": "#/components/schemas/ServerRef" }, { "type": "null" } ] },
                "either": { "oneOf": [ { "$ref": "#/components/schemas/Extra" }, { "type": "string" } ] },
                "merged": { "allOf": [ { "$ref": "#/components/schemas/ServerRef" }, { "$ref": "#/components/schemas/Extra" } ] }
            }
        });
        let out = inline_schema(&src, &c);
        assert!(!text(&out.schema).contains(COMPONENT_PREFIX));
        assert_eq!(out.schema["properties"]["nullable"]["anyOf"][0]["properties"]["server_id"]["type"], "string");
        assert_eq!(out.schema["properties"]["nullable"]["anyOf"][1]["type"], "null");
        assert_eq!(out.schema["properties"]["either"]["oneOf"][0]["properties"]["flag"]["type"], "boolean");
        assert_eq!(out.schema["properties"]["merged"]["allOf"][1]["properties"]["flag"]["type"], "boolean");
    }

    /// TEST-5 (acceptance, INV-1) — the whole point: a multi-level schema whose
    /// referenced types are reachable only through nested `properties` / `items`
    /// resolves to a document with ZERO component pointers and EVERY leaf field
    /// name present. Fails if any traversal arm is missed, and fails if a ref is
    /// merely dropped rather than expanded.
    #[test]
    fn acceptance_inv1_result_is_self_contained_and_complete() {
        let out = inline_schema(
            &json!({ "$ref": "#/components/schemas/CreateProjectRequest" }),
            &components(),
        );
        let s = text(&out.schema);
        assert_eq!(
            s.matches(COMPONENT_PREFIX).count(),
            0,
            "no #/components/ pointer may survive: {s}"
        );
        for leaf in ["name", "settings", "loop_limit", "servers", "server_id"] {
            assert!(s.contains(&format!("\"{leaf}\"")), "missing leaf `{leaf}` in {s}");
        }
        assert_eq!(out.form, SchemaForm::Inline);
        assert!(!out.truncated);
    }

    /// TEST-6 — a dangling ref degrades to a marker (never a panic, never a
    /// surviving pointer); a non-component ref is left byte-identical.
    #[test]
    fn dangling_ref_is_marked_and_foreign_ref_is_untouched() {
        let c = components();
        let out = inline_schema(
            &json!({ "type": "object", "properties": { "x": { "$ref": "#/components/schemas/Nope" } } }),
            &c,
        );
        let x = &out.schema["properties"]["x"];
        assert!(x.get("$ref").is_none(), "dangling pointer must not survive");
        assert!(
            x["$comment"].as_str().unwrap().contains("unresolved $ref"),
            "got {x}"
        );

        let foreign = json!({ "$ref": "https://example.test/schema.json#/Thing" });
        let out2 = inline_schema(&foreign, &c);
        assert_eq!(out2.schema, foreign, "a pointer we do not own is left alone");
    }

    /// TEST-7 (acceptance, INV-3) — a SELF-referential component terminates and
    /// is expressed as a real `$defs` self-reference, not truncated away.
    #[test]
    fn acceptance_inv3_self_recursive_schema_terminates() {
        let out = inline_schema(&json!({ "$ref": "#/components/schemas/Node" }), &components());
        assert_eq!(out.form, SchemaForm::Defs);
        // The recursive edge became a $defs pointer...
        assert_eq!(
            out.schema["properties"]["children"]["items"]["$ref"],
            json!("#/$defs/Node")
        );
        // ...and the target is present in the emitted document.
        assert!(out.schema["$defs"]["Node"].is_object());
        // The $defs entry references itself the same way — the closure is complete.
        assert_eq!(
            out.schema["$defs"]["Node"]["properties"]["children"]["items"]["$ref"],
            json!("#/$defs/Node")
        );
        assert!(!text(&out.schema).contains(COMPONENT_PREFIX));
        // The non-recursive sibling field is still fully present.
        assert_eq!(out.schema["properties"]["label"]["type"], "string");
    }

    /// TEST-8 (acceptance, INV-3) — mutual recursion terminates and both
    /// participants resolve inside the emitted document.
    #[test]
    fn acceptance_inv3_mutually_recursive_schemas_terminate() {
        let out = inline_schema(&json!({ "$ref": "#/components/schemas/A" }), &components());
        assert_eq!(out.form, SchemaForm::Defs);
        let s = text(&out.schema);
        assert!(!s.contains(COMPONENT_PREFIX), "{s}");
        let defs = out.schema["$defs"].as_object().expect("$defs");
        // Every #/$defs/N referenced anywhere resolves inside $defs.
        for name in ["A", "B"] {
            assert!(defs.contains_key(name), "missing $defs/{name}: {s}");
        }
        assert_eq!(defs["A"]["properties"]["b"]["$ref"], json!("#/$defs/B"));
        assert_eq!(defs["B"]["properties"]["a"]["$ref"], json!("#/$defs/A"));
    }

    /// TEST-9 — the depth and expansion bounds each defer instead of expanding,
    /// and report the cut via `form`.
    #[test]
    fn depth_and_expansion_budgets_defer_to_defs() {
        let c = components();
        let src = json!({ "$ref": "#/components/schemas/CreateProjectRequest" });

        let shallow = Budget { max_depth: 2, ..DEFAULT_BUDGET };
        let out = inline_schema_with(&src, &c, &shallow);
        assert_eq!(out.form, SchemaForm::Defs);
        assert!(!text(&out.schema).contains(COMPONENT_PREFIX));
        assert!(out.schema["$defs"].as_object().is_some_and(|d| !d.is_empty()));

        let narrow = Budget { max_expansions: 1, ..DEFAULT_BUDGET };
        let out2 = inline_schema_with(&src, &c, &narrow);
        assert_eq!(out2.form, SchemaForm::Defs);
        // Exactly one expansion happened: the root. Its nested ref was deferred.
        assert_eq!(
            out2.schema["properties"]["settings"]["$ref"],
            json!("#/$defs/ProjectSettings")
        );
        assert!(out2.schema["$defs"]["ProjectSettings"].is_object());
        assert!(!out2.truncated, "a $defs reference is not truncation");
    }

    /// TEST-10 (acceptance, INV-2) — over the soft byte cap the output degrades
    /// to the compact `$defs` form: still parseable, still self-contained, every
    /// `#/$defs/N` resolvable, and strictly smaller than the naive inline.
    #[test]
    fn acceptance_inv2_size_budget_degrades_to_a_valid_defs_document() {
        let c = components();
        // A schema that USES the same component many times — the case where
        // inlining multiplies size and the compact form does not.
        let mut props = Map::new();
        for i in 0..40 {
            props.insert(
                format!("f{i}"),
                json!({ "$ref": "#/components/schemas/CreateProjectRequest" }),
            );
        }
        let src = json!({ "type": "object", "properties": Value::Object(props) });

        let naive = inline_schema(&src, &c);
        let tight = Budget { max_bytes: 900, ..DEFAULT_BUDGET };
        let out = inline_schema_with(&src, &c, &tight);

        assert_eq!(out.form, SchemaForm::Defs);
        assert!(!out.truncated, "the soft cap must not elide anything");
        let s = text(&out.schema);
        assert!(
            serde_json::from_str::<Value>(&s).is_ok(),
            "degraded output must still be valid JSON"
        );
        assert!(!s.contains(COMPONENT_PREFIX), "still self-contained: {s}");
        assert!(
            s.len() < text(&naive.schema).len(),
            "compact form must be smaller than the naive inline ({} vs {})",
            s.len(),
            text(&naive.schema).len()
        );
        // Every $defs pointer resolves inside the document.
        let defs = out.schema["$defs"].as_object().expect("$defs");
        for name in referenced_defs(&out.schema) {
            assert!(defs.contains_key(&name), "dangling #/$defs/{name}: {s}");
        }
        // And the field contract is still readable through the $defs entry.
        assert_eq!(
            out.schema["$defs"]["CreateProjectRequest"]["properties"]["name"]["type"],
            "string"
        );
    }

    /// TEST-11 — past the HARD cap, whole `$defs` entries are elided to named
    /// placeholders, deterministically, and the document still parses.
    #[test]
    fn hard_cap_elides_whole_defs_entries_and_flags_truncation() {
        let c = components();
        let src = json!({ "$ref": "#/components/schemas/CreateProjectRequest" });
        let tiny = Budget {
            max_bytes: 10,
            hard_max_bytes: 220,
            ..DEFAULT_BUDGET
        };
        let out = inline_schema_with(&src, &c, &tiny);
        assert_eq!(out.form, SchemaForm::Defs);
        assert!(out.truncated, "elision must be reported: {}", text(&out.schema));

        let s = text(&out.schema);
        assert!(serde_json::from_str::<Value>(&s).is_ok(), "still valid JSON: {s}");
        assert!(!s.contains(COMPONENT_PREFIX), "{s}");
        let defs = out.schema["$defs"].as_object().expect("$defs");
        // Entries are NAMED even when elided — the model still learns the type
        // exists, and every reference still resolves.
        for name in referenced_defs(&out.schema) {
            assert!(defs.contains_key(&name), "dangling #/$defs/{name}: {s}");
        }
        assert!(
            defs.values().any(|v| v
                .get("$comment")
                .and_then(|c| c.as_str())
                .is_some_and(|c| c.contains("omitted for size"))),
            "expected at least one elided placeholder: {s}"
        );

        // Deterministic: the same input yields byte-identical output.
        let again = inline_schema_with(&src, &c, &tiny);
        assert_eq!(text(&again.schema), s);
    }

    /// Every `#/$defs/N` name referenced anywhere in `v`.
    fn referenced_defs(v: &Value) -> Vec<String> {
        let mut out = Vec::new();
        fn rec(v: &Value, out: &mut Vec<String>) {
            match v {
                Value::Object(o) => {
                    for (k, val) in o {
                        if k == "$ref"
                            && let Some(n) = val.as_str().and_then(|s| s.strip_prefix(DEFS_PREFIX))
                        {
                            out.push(n.to_string());
                        }
                        rec(val, out);
                    }
                }
                Value::Array(a) => a.iter().for_each(|x| rec(x, out)),
                _ => {}
            }
        }
        rec(v, &mut out);
        out
    }

    /// The single-hop resolver `validate_body` uses: resolves a known name and
    /// FAILS OPEN on anything else (the behaviour the removed duplicate had).
    #[test]
    fn resolve_schema_ref_is_single_hop_and_fails_open() {
        let c = components();
        let r = resolve_schema_ref(&json!({ "$ref": "#/components/schemas/ServerRef" }), &c);
        assert_eq!(r["type"], "object");
        let plain = json!({ "type": "string" });
        assert_eq!(resolve_schema_ref(&plain, &c), plain);
        let dangling = json!({ "$ref": "#/components/schemas/Nope" });
        assert_eq!(
            resolve_schema_ref(&dangling, &c),
            dangling,
            "unknown name returns the input unchanged (fail open)"
        );
    }

    /// A ref-free schema is passed through byte-identically, whatever its size —
    /// the budget bounds EXPANSION, not the operation's own declared schema.
    #[test]
    fn ref_free_schema_is_passed_through_unchanged() {
        let src = json!({
            "type": "object",
            "required": ["a"],
            "additionalProperties": false,
            "properties": { "a": { "type": "string", "enum": ["x", "y"], "default": "x" } }
        });
        let out = inline_schema(&src, &json!({}));
        assert_eq!(out.schema, src);
        assert_eq!(out.form, SchemaForm::Inline);
        assert!(!out.truncated);
    }

    /// A `$ref` carrying sibling annotations keeps them (the per-use
    /// `description` is exactly the text the model needs), without letting them
    /// override the referenced schema's own keys.
    #[test]
    fn sibling_annotations_beside_a_ref_are_preserved() {
        let out = inline_schema(
            &json!({
                "$ref": "#/components/schemas/ServerRef",
                "description": "The server to attach",
                "type": "IGNORED"
            }),
            &components(),
        );
        assert_eq!(out.schema["description"], "The server to attach");
        assert_eq!(out.schema["type"], "object", "the target's own key wins");
    }
}
