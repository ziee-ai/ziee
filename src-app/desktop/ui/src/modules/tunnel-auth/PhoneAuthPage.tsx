/**
 * PhoneAuthPage — rendered by the desktop AuthGuard when there's no
 * Tauri webview (i.e. the bundle was loaded over the ngrok tunnel)
 * and the user isn't authenticated.
 *
 * Delegates fetching `/api/auth/config` and submitting password
 * login to the TunnelAuth store; this component is presentational.
 *
 * Branches on `authConfig.password_auth_enabled`:
 *   - true → renders a password-only form (no username field — the
 *     remote-access surface is single-admin, surfacing a username
 *     would just leak the admin's name). A hidden username anchor
 *     is included so password managers can save the credential.
 *   - false → "use the magic link from the desktop app" message,
 *     no form.
 *
 * If a 403 PASSWORD_LOGIN_DISABLED comes back mid-submit (admin
 * toggled it off while the phone user was typing), we re-fetch the
 * config and let the branch flip to the no-form path.
 */

import { useEffect, useState } from 'react'
import { Alert, Button, Card, Form, Input, Result, Spin, theme, Typography } from 'antd'
import { useTunnelAuthStore } from './TunnelAuth.store'

const { Title, Paragraph } = Typography

/**
 * Wrap the screen in antd's theme-aware Layout background so the
 * Card's `colorBgContainer` sits on a matching `colorBgLayout`.
 * Without this the bare `<div>` falls through to browser-white and
 * the Card visibly "floats" on a wrong-color page.
 */
function PageShell({ children }: { children: React.ReactNode }) {
  const { token } = theme.useToken()
  return (
    <div
      className="min-h-screen flex items-center justify-center p-4"
      style={{ backgroundColor: token.colorBgLayout }}
    >
      {children}
    </div>
  )
}

export function PhoneAuthPage() {
  const authConfig = useTunnelAuthStore(s => s.authConfig)
  const loadingConfig = useTunnelAuthStore(s => s.loadingConfig)
  const configError = useTunnelAuthStore(s => s.configError)
  const submitting = useTunnelAuthStore(s => s.submittingPassword)
  const submitError = useTunnelAuthStore(s => s.passwordError)

  const [password, setPassword] = useState('')

  useEffect(() => {
    void useTunnelAuthStore.getState().loadAuthConfig()
  }, [])

  const onSubmit = async () => {
    try {
      await useTunnelAuthStore.getState().phonePasswordLogin(password.trim())
      // AuthGuard re-renders children automatically once
      // isAuthenticated flips — no navigate needed.
    } catch (e) {
      // 403 PASSWORD_LOGIN_DISABLED means the admin disabled
      // password auth while the user was typing. Re-fetch config so
      // the no-form branch renders.
      const msg = e instanceof Error ? e.message : String(e)
      if (/PASSWORD_LOGIN_DISABLED|disabled/i.test(msg)) {
        void useTunnelAuthStore.getState().loadAuthConfig()
      }
    }
  }

  if (configError && !authConfig) {
    return (
      <PageShell>
        <Result
          status="warning"
          title="Couldn't load login options"
          subTitle={configError}
        />
      </PageShell>
    )
  }

  if (loadingConfig || !authConfig) {
    return (
      <PageShell>
        <Spin size="large" />
      </PageShell>
    )
  }

  if (!authConfig.password_auth_enabled) {
    return (
      <PageShell>
        <Card className="max-w-md w-full">
          <Title level={4}>Open the desktop app</Title>
          <Paragraph>
            This device can only sign in via a fresh magic-link from the
            desktop app. Open the Remote Access page on your desktop and
            scan the QR code (or copy the link) to log in.
          </Paragraph>
        </Card>
      </PageShell>
    )
  }

  return (
    <PageShell>
      <Card className="max-w-md w-full">
        <Title level={4}>Sign in</Title>
        <Paragraph type="secondary">
          Enter the password set on this device's desktop app.
        </Paragraph>
        <Form layout="vertical" onFinish={onSubmit}>
          {/* Hidden username anchor so password managers
              (1Password / Chrome / Safari Keychain) can attach the
              saved credential to this host. No visible field — the
              admin's actual username stays hidden, matching the
              `hide_username` config the backend sets in tunnel mode. */}
          <input
            type="text"
            autoComplete="username"
            value="admin"
            readOnly
            hidden
          />
          <Form.Item
            label="Password"
            name="password"
            rules={[{ required: true, message: 'Enter your password' }]}
          >
            <Input.Password
              value={password}
              onChange={e => setPassword(e.target.value)}
              autoFocus
              autoComplete="current-password"
              aria-label="Password"
            />
          </Form.Item>
          {submitError && (
            <Form.Item>
              <Alert type="error" title={submitError} showIcon />
            </Form.Item>
          )}
          <Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              block
              loading={submitting}
              disabled={!password.trim()}
            >
              Sign in
            </Button>
          </Form.Item>
        </Form>
      </Card>
    </PageShell>
  )
}
