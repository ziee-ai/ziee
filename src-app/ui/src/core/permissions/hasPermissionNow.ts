// See .claude/PERMISSION_GATING.md for the gating pattern and the
// checklist for new features.

import { useAuthStore } from '@/modules/auth/Auth.store'
import { evaluatePermission } from './evaluatePermission'
import type { PermissionExpr } from './types'

/**
 * Non-reactive permission check, safe to call from store `__init__`
 * hooks, actions, event handlers, or any other code path that runs
 * outside a React component body.
 *
 * Reads `user` + `permissions` directly from the underlying Zustand
 * store via `useAuthStore.getState()`, so it does NOT subscribe to
 * changes. If a component needs to re-render when permissions
 * change, use `usePermission()` or `<Can>` instead.
 *
 * Primary use case: gating shell-eager-load fetches (audit
 * follow-up) — modules whose `__init__` calls `/api/...` for
 * resources the user may not have access to. Without the gate, the
 * shell 403s on every render for permission-restricted users.
 */
export function hasPermissionNow(expr: PermissionExpr): boolean {
  const { user, permissions } = useAuthStore.getState()
  return evaluatePermission(user, permissions, expr)
}
