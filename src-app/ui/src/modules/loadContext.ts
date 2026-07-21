import { Auth } from '@/modules/auth/Auth.store'
import { App } from '@/modules/app/stores/app'
import { evaluatePermission } from '@/core/permissions'
import type { ModuleLoadContext } from '@ziee/framework/module-system'

/**
 * Build the {@link ModuleLoadContext} the manifest predicates evaluate against,
 * from the CURRENT (non-reactive) snapshot of the core Auth + App stores. The
 * loader rebuilds it on every relevant change (login, permission grant, setup
 * completion) and re-selects eligible modules.
 *
 * `can(...perms)` mirrors `hasPermissionNow` / `<Can>` — it evaluates each perm
 * through `evaluatePermission` (is_admin wildcard short-circuit) against the SAME
 * snapshot used for `permissions`, so a predicate written `ctx.can(Permissions.X)`
 * behaves identically to the route/slot `permission` gate. Requires ALL given
 * perms (AND); use one `can()` per required perm for OR at the call site.
 */
export function buildLoadContext(pathname: string): ModuleLoadContext {
  const auth = Auth.$
  const app = App.$
  const user = auth.user
  const permissions = auth.permissions ?? []
  return {
    isAuthenticated: !!auth.isAuthenticated,
    needsSetup: app.needsSetup === true,
    path: pathname,
    permissions,
    platform:
      typeof window !== 'undefined' &&
      (window as unknown as { __TAURI__?: unknown }).__TAURI__
        ? 'desktop'
        : 'web',
    can: (...perms: string[]) =>
      perms.every(p => evaluatePermission(user, permissions, p)),
  }
}
