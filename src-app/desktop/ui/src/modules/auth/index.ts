/**
 * Desktop shadow of `@/modules/auth/index.ts`.
 *
 * Core's barrel re-exports `AuthGuard` via a RELATIVE path
 * (`export { AuthGuard } from './AuthGuard'`), which the localOverridePlugin
 * does NOT intercept — so if anything ever imports `AuthGuard` through the
 * `@/modules/auth` barrel form, the core barrel would bind core's AuthGuard even
 * on desktop, bypassing the `AuthGuard.desktop.tsx` tier-2 override. This shadow
 * re-exports `AuthGuard` via the `@/modules/auth/AuthGuard` specifier so the
 * resolver picks the `.desktop` file. It is DEFENSIVE: the live render path
 * (`ui/src/modules/auth/module.tsx`) already imports the `@/…/AuthGuard` form
 * directly, so the override works without this barrel today — but the barrel must
 * stay BYTE-COMPLETE with core's (below) so a future `@/modules/auth`-form import
 * of any symbol doesn't silently resolve to a partial shadow on desktop.
 */
export { AuthGuard } from '@/modules/auth/AuthGuard'
// The rest of core's auth surface, verbatim — keep in lockstep with
// `ui/src/modules/auth/index.ts`.
export { AuthPage } from '@ziee/ui-core/modules/auth/AuthPage'
export { LoginForm } from '@ziee/ui-core/modules/auth/LoginForm'
export { RegisterForm } from '@ziee/ui-core/modules/auth/RegisterForm'
export { useAuthStore } from '@ziee/ui-core/modules/auth/Auth.store'
