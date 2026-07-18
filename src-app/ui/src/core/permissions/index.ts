// Shim: re-exports the permission primitives now living in
// `@ziee/framework/permissions`. See ./types for the enum-narrowed `Permission`.
//
// See .claude/PERMISSION_GATING.md for the gating pattern and the checklist for
// new features.

export { Can } from './Can'
export { evaluatePermission } from './evaluatePermission'
export { hasPermission } from './hasPermission'
export { hasPermissionNow } from './hasPermissionNow'
export { usePermission } from './usePermission'
export type { Permission, PermissionExpr } from './types'
