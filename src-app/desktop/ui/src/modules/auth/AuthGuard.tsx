/**
 * DELIBERATE DIVERGENCE from core's AuthGuard.
 *
 * Differs from the core AuthGuard in four ways:
 *
 *  1. NEVER renders <AuthPage />. Desktop is a single-admin app whose
 *     credentials live in a Tauri command (`auto_login` in commands.rs);
 *     surfacing the multi-user username/password form would just trap
 *     the Tauri user because they don't know the hardcoded password.
 *
 *  2. NEVER redirects to /setup. The Tauri `setup` hook calls
 *     `ensure_desktop_admin()` BEFORE the webview is created, so
 *     `needs_setup` should always be false. If the App store races with
 *     server bootstrap on the very first launch (returning `null` /
 *     `true` momentarily), we just keep spinning rather than briefly
 *     flashing the setup page.
 *
 *  3. Skips `Stores.Auth.initAuth()`. The desktop-base module's
 *     auto-login retry loop is the single source of truth for the
 *     token; any persisted token from a previous launch is stale
 *     because the server regenerates its JWT secret per launch
 *     (see desktop/tauri/src/modules/backend/mod.rs).
 *
 *  4. Branches on `isTauriView` when not authenticated. The same
 *     desktop bundle is served to phones via the ngrok tunnel — there
 *     IS no Tauri webview there, so the auto-login spinner would hang
 *     forever. Phone callers see <PhoneAuthPage /> (password form or
 *     "open desktop for a magic link" message, depending on the
 *     admin's Remote Access config).
 *
 * Spinner caption (Tauri path) is driven by the bootstrap store so
 * the user sees "Starting up…" → "Backend failed to start. Try
 * restarting Ziee." instead of an indefinite, contextless spin.
 *
 * NOTE: The core auth no longer auto-redirects to onboarding — that
 * logic moved into `ui/src/modules/onboarding/OnboardingRedirect.tsx`,
 * which the onboarding module registers into the `routerEffects`
 * slot. The redirect there is admin-gated, so the desktop tunnel
 * surface (which always logs in as admin) doesn't get trapped in an
 * onboarding loop it can't escape.
 */

import { Layout, Spin, Typography } from 'antd'
import { Stores } from '@/core/stores'
import { useBootstrapStore } from '@ziee/desktop/modules/desktop-base/Bootstrap.store'
import { isTauriView } from '@ziee/desktop/core/platform'
import { PhoneAuthPage } from '@ziee/desktop/modules/tunnel-auth/PhoneAuthPage'

const { Content } = Layout

interface AuthGuardProps {
  children: React.ReactNode
}

export const AuthGuard: React.FC<AuthGuardProps> = ({ children }) => {
  const { isAuthenticated } = Stores.Auth
  const bootstrapStatus = useBootstrapStore(s => s.status)
  const bootstrapMessage = useBootstrapStore(s => s.message)

  if (!isAuthenticated) {
    // Phone-over-tunnel: no Tauri shell, no auto-login coming.
    // Render the phone login surface (password form or "use the
    // magic link from the desktop app" message).
    if (!isTauriView) {
      return <PhoneAuthPage />
    }

    // Tauri webview: bootstrap-aware spinner. If auto-login truly
    // failed, surface the actionable message but stay on the spinner
    // shell — restarting the app is the only recovery path.
    const caption =
      bootstrapStatus === 'failed'
        ? bootstrapMessage ?? 'Backend failed to start. Try restarting Ziee.'
        : bootstrapMessage ?? 'Starting up…'
    return (
      <Layout className="min-h-screen">
        <Content className="flex flex-col items-center justify-center gap-4">
          <Spin size="large" />
          <Typography.Text
            type={bootstrapStatus === 'failed' ? 'danger' : 'secondary'}
          >
            {caption}
          </Typography.Text>
        </Content>
      </Layout>
    )
  }

  return <>{children}</>
}
