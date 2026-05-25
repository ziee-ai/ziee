// See .claude/PERMISSION_GATING.md for the gating pattern and the
// checklist for new features.

/**
 * Composable permission expression. Used by every gating surface
 * (slot fields, <Can> component, usePermission hook, evaluator).
 *
 * - bare string: an exact permission, matched with the same wildcard
 *   rules as the backend (`*`, `module::resource::*`, etc.).
 * - `allOf`: every child expression must pass (AND).
 * - `anyOf`: at least one child expression must pass (OR).
 *
 * Names mirror JSON Schema / OpenAPI conventions used elsewhere in
 * this project. The shape is intentionally serializable (no functions)
 * so it can flow through slot registrations and be inspected by
 * tooling.
 */
export type PermissionExpr =
  | string
  | { allOf: PermissionExpr[] }
  | { anyOf: PermissionExpr[] }
