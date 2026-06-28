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

import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Form,
  FormField,
  PasswordInput,
  Result,
  Spin,
  Title,
  Paragraph,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { useTunnelAuthStore } from './TunnelAuth.store'

const passwordSchema = z.object({
  password: z.string().min(1, 'Enter your password'),
})
type PasswordFormValues = z.infer<typeof passwordSchema>

/**
 * Wrap the screen in the theme-aware layout background so the
 * Card sits on a matching page color. Without this the bare `<div>`
 * falls through to browser-white and the Card visibly "floats" on a
 * wrong-color page.
 */
function PageShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen flex items-center justify-center p-4 bg-background">
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

  const form = useForm<PasswordFormValues>({
    resolver: zodResolver(passwordSchema),
    defaultValues: { password: '' },
  })
  const password = (form.watch('password') ?? '').trim()

  useEffect(() => {
    void useTunnelAuthStore.getState().loadAuthConfig()
  }, [])

  const onSubmit = async (values: PasswordFormValues) => {
    try {
      await useTunnelAuthStore.getState().phonePasswordLogin(values.password.trim())
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
          data-testid="desktop-tunnel-phone-config-error-result"
          status="warning"
          title="Couldn't load login options"
          subtitle={configError}
        />
      </PageShell>
    )
  }

  if (loadingConfig || !authConfig) {
    return (
      <PageShell>
        <Spin size="lg" label="Loading" />
      </PageShell>
    )
  }

  if (!authConfig.password_auth_enabled) {
    return (
      <PageShell>
        <Card data-testid="desktop-tunnel-phone-magic-only-card" className="max-w-md w-full">
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
      <Card data-testid="desktop-tunnel-phone-signin-card" className="max-w-md w-full">
        <Title level={4}>Sign in</Title>
        <Paragraph type="secondary">
          Enter the password set on this device's desktop app.
        </Paragraph>
        <Form data-testid="desktop-tunnel-phone-password-form" form={form} onSubmit={onSubmit} layout="vertical">
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
          <FormField label="Password" name="password">
            <PasswordInput
              data-testid="desktop-tunnel-phone-password-input"
              autoFocus
              autoComplete="current-password"
              aria-label="Password"
              showLabel="Show password"
              hideLabel="Hide password"
            />
          </FormField>
          {submitError && (
            <Alert data-testid="desktop-tunnel-phone-submit-error-alert" tone="error" title={submitError} />
          )}
          <Button
            data-testid="desktop-tunnel-phone-signin-btn"
            type="submit"
            block
            loading={submitting}
            disabled={!password}
          >
            Sign in
          </Button>
        </Form>
      </Card>
    </PageShell>
  )
}
