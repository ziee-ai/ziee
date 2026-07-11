/**
 * Desktop override of `@/modules/auth/index.ts`.
 *
 * Why this file exists: core's index does
 *   `export { AuthGuard } from './AuthGuard'`
 * with a relative path, so `import { AuthGuard } from '@/modules/auth'`
 * (the form used in `ui/src/modules/router/components/RouterComponent.tsx`)
 * resolves through core's index → core's AuthGuard, BYPASSING the
 * `desktop/ui/src/modules/auth/AuthGuard.tsx` override the
 * localOverridePlugin would otherwise apply.
 *
 * Without this file, the desktop AuthGuard never gets used and the
 * SPA falls back to core's auth flow (AuthPage, /setup redirect, …)
 * even in the Tauri webview.
 *
 * The override itself was relocated out of the desktop tree into the
 * core tree as `ui/src/modules/auth/AuthGuard.desktop.tsx` (the
 * localOverridePlugin's tier-2 co-located shadow). Importing it via the
 * `@/modules/auth/AuthGuard` specifier lets that resolver pick the
 * `.desktop` file for the desktop bundle — the same way core's own
 * `ui/src/modules/auth/module.tsx` imports it.
 */

export { AuthGuard } from '@/modules/auth/AuthGuard'

// Re-export the rest of core's auth surface verbatim — we only need
// to swap AuthGuard.
export { AuthPage } from '@ziee/ui-core/modules/auth/AuthPage'
export { LoginForm } from '@ziee/ui-core/modules/auth/LoginForm'
export { RegisterForm } from '@ziee/ui-core/modules/auth/RegisterForm'
