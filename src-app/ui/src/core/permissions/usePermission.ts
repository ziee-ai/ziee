// See .claude/PERMISSION_GATING.md for the gating pattern and the
// checklist for new features.

import { Stores } from '@/core/stores'
import { evaluatePermission } from './evaluatePermission'
import type { PermissionExpr } from './types'

/**
 * Returns whether the current authenticated user satisfies the given
 * permission expression. Reads `user` + `permissions` reactively from
 * `Stores.Auth` (populated by `initAuth` at app boot).
 *
 * Composition lives in the expression, not in the hook name — pass
 * `{ allOf: [...] }` for AND or `{ anyOf: [...] }` for OR rather
 * than reaching for separate `useAllPermissions` / `useAnyPermission`
 * hooks.
 */
export function usePermission(expr: PermissionExpr): boolean {
  const { user, permissions } = Stores.Auth
  return evaluatePermission(user, permissions, expr)
}
