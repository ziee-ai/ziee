// See .claude/PERMISSION_GATING.md for the gating pattern and the
// checklist for new features.

import type { User } from '@/api-client/types'
import { hasPermission } from './hasPermission'
import type { PermissionExpr } from './types'

/**
 * Walk a `PermissionExpr` tree and evaluate it against the user's
 * permissions. Bare strings delegate to `hasPermission` (the leaf
 * check). `allOf` is AND, `anyOf` is OR. Empty `allOf` is vacuously
 * true; empty `anyOf` is false.
 */
export function evaluatePermission(
  user: User | null | undefined,
  permissions: string[] | null | undefined,
  expr: PermissionExpr,
): boolean {
  if (typeof expr === 'string') {
    return hasPermission(user, permissions, expr)
  }
  if ('allOf' in expr) {
    return expr.allOf.every(child =>
      evaluatePermission(user, permissions, child),
    )
  }
  if ('anyOf' in expr) {
    return expr.anyOf.some(child =>
      evaluatePermission(user, permissions, child),
    )
  }
  return false
}
