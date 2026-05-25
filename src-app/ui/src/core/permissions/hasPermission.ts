// See .claude/PERMISSION_GATING.md for the gating pattern and the
// checklist for new features.

import type { User } from '@/api-client/types'

/**
 * Leaf permission check — mirrors the backend's
 * `permissions/checker.rs::check_permissions_array` exactly, plus the
 * `is_admin` short-circuit that the backend applies one level up at
 * `permissions/extractors.rs`.
 *
 * Order matters:
 * 1. `user.is_admin === true` → granted (root admin bypass — see
 *    `.claude/PERMISSION_GATING.md` on root admin vs Administrators
 *    group; the /api/auth/me payload does NOT rewrite permissions[]
 *    to ["*"] for root admins, so we must short-circuit here).
 * 2. exact match in permissions[].
 * 3. `*` global wildcard in permissions[].
 * 4. hierarchical `::` wildcard: for `a::b::c`, check `a::*` and
 *    `a::b::*`. Separator is double-colon, matching the backend; an
 *    earlier sandbox-local helper used single-colon and silently
 *    failed wildcard matching as a result.
 */
export function hasPermission(
  user: User | null | undefined,
  permissions: string[] | null | undefined,
  required: string,
): boolean {
  if (user?.is_admin) return true

  if (!permissions || permissions.length === 0) return false

  if (permissions.includes(required)) return true
  if (permissions.includes('*')) return true

  const parts = required.split('::')
  for (let i = 1; i < parts.length; i++) {
    const wildcard = parts.slice(0, i).join('::') + '::*'
    if (permissions.includes(wildcard)) return true
  }

  return false
}
