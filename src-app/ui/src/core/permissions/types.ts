// Shim: the permission primitives moved to `@ziee/framework/permissions`.
// This app-side path re-exports them so existing `@/core/permissions` imports
// compile unchanged, while narrowing the leaf `Permission` to ziee's generated
// `Permissions` enum for enum-level type-safety.
//
// See .claude/PERMISSION_GATING.md for the gating pattern and the checklist for
// new features.

import type { Permissions } from '@/api-client/types'

/**
 * A single permission, restricted to the `Permissions` enum emitted by the
 * OpenAPI type generator. Pass enum members for type-safety:
 *
 *   import { Permissions } from '@/api-client/types'
 *   permission: Permissions.UsersDelete
 */
export type Permission = Permissions

export type { PermissionExpr, PermissionUser } from '@ziee/framework/permissions'
