/**
 * MagicLinkPage — consumer side of the remote-access magic-link.
 *
 * Lives in the DESKTOP UI bundle (no server-UI changes). Reached
 * only when a phone user opens the link the admin generated from
 * the desktop Remote Access page. Delegates to the TunnelAuth
 * store's `exchangeMagicLink` action which (a) dedupes Strict-Mode
 * double-mount + browser-refresh, (b) seeds `Stores.Auth`, (c)
 * captures any error onto a single store slot.
 *
 * Route is registered `requiresAuth: false` so it renders WITHOUT
 * the AuthGuard wrap — otherwise the guard would intercept the
 * unauthenticated phone and show the password-login shell instead
 * of running the exchange.
 */

import { useEffect } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { Button, Result, Spin, theme } from 'antd'
import { Stores } from '@/core/stores'
import { useTunnelAuthStore } from './TunnelAuth.store'

/**
 * Match the page chrome to antd's theme-aware layout background so
 * the Result / Spin doesn't sit on browser-white when the antd theme
 * is dark.
 */
function PageShell({
  children,
  centerColumn,
}: {
  children: React.ReactNode
  centerColumn?: boolean
}) {
  const { token } = theme.useToken()
  return (
    <div
      className={`min-h-screen flex items-center justify-center p-4 ${
        centerColumn ? 'flex-col gap-3' : ''
      }`}
      style={{ backgroundColor: token.colorBgLayout }}
    >
      {children}
    </div>
  )
}

export function MagicLinkPage() {
  const { token } = useParams<{ token: string }>()
  const navigate = useNavigate()

  const exchangingToken = useTunnelAuthStore(s => s.exchangingToken)
  const exchangeError = useTunnelAuthStore(s => s.exchangeError)

  useEffect(() => {
    if (!token) return
    let unmounted = false
    void (async () => {
      try {
        await useTunnelAuthStore.getState().exchangeMagicLink(token)
        if (unmounted) return
        // Replace so the spent token URL never lands in browser history.
        navigate('/', { replace: true })
      } catch {
        // Already captured into `exchangeError` by the store.
      }
    })()
    return () => {
      unmounted = true
    }
  }, [token, navigate])

  if (!token) {
    return (
      <PageShell>
        <Result
          status="warning"
          title="Missing token"
          subTitle="This URL doesn't include a magic-link token. Open the desktop Remote Access page and scan a fresh QR."
        />
      </PageShell>
    )
  }

  if (exchangingToken === token && !exchangeError) {
    return (
      <PageShell centerColumn>
        <Spin size="large" />
        <span className="text-sm text-gray-500">Logging you in…</span>
      </PageShell>
    )
  }

  if (exchangeError) {
    // Refresh-after-success special case: the token has been consumed
    // server-side and the store's previous successful exchange already
    // pushed the session into Stores.Auth (still in memory). If we're
    // already authenticated, just bounce home rather than show the
    // "link expired" page — the user is already logged in.
    if (Stores.Auth.__state.isAuthenticated) {
      navigate('/', { replace: true })
      return null
    }
    return (
      <PageShell>
        <Result
          status="error"
          title="Link no longer valid"
          subTitle={
            <>
              <div>{exchangeError}</div>
              <div className="text-xs text-gray-500 mt-2">
                Magic links expire after 5 minutes and can only be used once.
                Get a fresh one from the desktop app's Remote Access page.
              </div>
            </>
          }
          extra={
            <Button type="primary" onClick={() => navigate('/', { replace: true })}>
              Open login page
            </Button>
          }
        />
      </PageShell>
    )
  }

  // Shouldn't normally render; covers the brief window before the
  // useEffect fires.
  return (
    <PageShell>
      <Spin size="large" />
    </PageShell>
  )
}
