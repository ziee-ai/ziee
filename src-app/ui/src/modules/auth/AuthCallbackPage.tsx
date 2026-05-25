import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Alert, Card, Layout, Spin, Typography } from 'antd'
import { Stores } from '@/core/stores'
import { BlankLayoutComponent } from '@/modules/layouts/blank'

const { Content } = Layout
const { Title, Text } = Typography
const SESSION_RETURN_TO_KEY = 'ziee.oauth.returnTo'

/**
 * /auth/callback — landing page after a successful OAuth dance.
 *
 * The backend redirects to `/auth/callback#token=<JWT>&return_to=<path>`.
 * The token is in the URL **fragment** (`#…`) so it's never sent in
 * Referer headers or written to server access logs. We:
 *   1. Read both fields from `location.hash`.
 *   2. Call `history.replaceState` to scrub them immediately —
 *      removes the token from URL bar, browser history, and any
 *      subsequent Referer.
 *   3. Hand the token to `Auth.store.setAuthFromAutoLogin`, then call
 *      `Auth.initAuth()` to fetch `/api/auth/me` and hydrate user +
 *      permissions.
 *   4. Navigate to `return_to` (URL > sessionStorage > "/").
 */
export const AuthCallbackPage: React.FC = () => {
  const navigate = useNavigate()
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    const run = async () => {
      const fragment = window.location.hash.startsWith('#')
        ? window.location.hash.slice(1)
        : window.location.hash
      const params = new URLSearchParams(fragment)
      const token = params.get('token')
      const returnToFromUrl = params.get('return_to')

      // Scrub the fragment FIRST so even an immediate throw below
      // doesn't leave the token in the URL bar.
      try {
        window.history.replaceState({}, '', '/auth/callback')
      } catch {
        // ignore — older browsers / sandboxed iframes
      }

      if (!token) {
        setError('No authentication token in the callback URL.')
        return
      }

      // Hydrate the auth store + re-fetch profile.
      Stores.Auth.setAuthFromAutoLogin({
        user: null as any, // server will be the truth — see initAuth below
        access_token: token,
        refresh_token: '',
      })

      try {
        await Stores.Auth.initAuth()
      } catch (e) {
        if (!cancelled) {
          setError(
            e instanceof Error ? e.message : 'Failed to load user profile',
          )
        }
        return
      }

      // Resolve the post-login destination. URL value wins; fall back
      // to sessionStorage (set on the LoginForm button click); fall
      // back to "/".
      let target = returnToFromUrl ?? null
      if (!target) {
        try {
          target = window.sessionStorage.getItem(SESSION_RETURN_TO_KEY)
        } catch {
          target = null
        }
      }
      try {
        window.sessionStorage.removeItem(SESSION_RETURN_TO_KEY)
      } catch {
        // ignore
      }

      if (!cancelled) {
        navigate(target || '/', { replace: true })
      }
    }
    run()
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <BlankLayoutComponent>
      <Layout className="min-h-screen">
        <Content className="flex items-center justify-center p-4">
          <Card className="w-full max-w-md text-center">
            {error ? (
              <>
                <Title level={4}>Sign-in failed</Title>
                <Alert type="error" message={error} showIcon className="my-3" />
                <a href="/login">Return to login</a>
              </>
            ) : (
              <>
                <Spin size="large" />
                <div className="mt-4">
                  <Text type="secondary">Completing sign-in…</Text>
                </div>
              </>
            )}
          </Card>
        </Content>
      </Layout>
    </BlankLayoutComponent>
  )
}
