/**
 * Desktop Override: AuthGuard
 *
 * Differs from the core AuthGuard in three ways:
 *
 *  1. NEVER renders <AuthPage />. Desktop is a single-admin app whose
 *     credentials live in a Tauri command (`auto_login` in commands.rs);
 *     surfacing the username/password form would just trap the user
 *     because they don't know the hardcoded password.
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
 * Spinner caption is driven by the bootstrap store so the user sees
 * "Starting up…" → "Backend failed to start. Try restarting Ziee."
 * instead of an indefinite, contextless spin.
 *
 * NOTE: The core AuthGuard also auto-redirects users with incomplete
 * onboarding guides. That branch is omitted here because the desktop
 * api-client's User type snapshot is out of sync with core (missing
 * `completed_onboarding_ids`); users can still navigate to onboarding
 * routes manually, and re-introducing the redirect should land in the
 * same change that regenerates the desktop's openapi types.
 */

import { Layout, Spin, Typography } from 'antd'
import { Stores } from '@/core/stores'
import { useBootstrapStore } from '@ziee/desktop/modules/desktop-base/Bootstrap.store'

const { Content } = Layout

interface AuthGuardProps {
  children: React.ReactNode
}

export const AuthGuard: React.FC<AuthGuardProps> = ({ children }) => {
  const { isAuthenticated } = Stores.Auth
  const bootstrapStatus = useBootstrapStore(s => s.status)
  const bootstrapMessage = useBootstrapStore(s => s.message)

  // Not yet authenticated: show a bootstrap-aware spinner. Never AuthPage,
  // never /setup. If auto-login truly failed, surface the actionable
  // message but stay on the spinner shell — restarting the app is the
  // only recovery path.
  if (!isAuthenticated) {
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
