/**
 * Remote Access settings page (DESKTOP-ONLY).
 *
 * Drives the full setup flow on a single page:
 *   1. Save ngrok auth token (masked input).
 *   2. Optional custom domain (paid-plan reserved subdomain).
 *   3. Auto-start switch (only visible when a domain is set).
 *   4. Password authentication toggle (default OFF; turning it ON
 *      shows an inline password-set form when the admin password
 *      is still the bootstrap default).
 *   5. Start / stop tunnel; once connected, render the QR code +
 *      plaintext magic-link URL with Copy buttons + countdown
 *      until rotation.
 *
 * Lives ONLY in the desktop bundle — phones hitting the tunnel
 * never receive this code path, so they can't disable the tunnel
 * they're using.
 */

import {
  Alert,
  Button,
  Card,
  Empty,
  Form,
  FormField,
  Input,
  PasswordInput,
  Separator,
  Space,
  Switch,
  Tag,
  Tooltip,
  Title,
  Text,
  Paragraph,
  message,
  useForm,
  zodResolver,
} from '@ziee/kit'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldLabel,
} from '@ziee/kit/shadcn/field'
import { CircleCheck, Copy, RotateCw, TriangleAlert } from 'lucide-react'
import { z } from 'zod'
import { QRCodeSVG } from 'qrcode.react'
import { useEffect, useMemo, useState } from 'react'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

export function RemoteAccessPage() {
  const { status, loading, saving, error, magicLink } = Stores.RemoteAccess

  // Local form state (uncontrolled by the store so the user can
  // type without each keystroke firing a save). Persisted only on
  // Save click.
  const [tokenDraft, setTokenDraft] = useState('')
  const [domainDraft, setDomainDraft] = useState('')

  useEffect(() => {
    setDomainDraft(status?.ngrok_domain ?? '')
  }, [status?.ngrok_domain])

  // Safety net: the store's __init__ fires once on first store-touch,
  // which can race with the desktop auto-login (Tauri webview opens
  // the page route before the JWT lands in localStorage). If that
  // first call 401s the page is stuck on the empty state forever.
  // Always re-fetch on mount so a stale failed init self-heals.
  //
  // On unmount, stop the rotation timer — otherwise the 4-min
  // setInterval keeps firing forever in the background after the
  // user navigates away, hitting `/issue` every 4 min and bloating
  // the magic_link_tokens table. The next mount of this page
  // (loadStatus → tunnel still Connected) restarts rotation.
  useEffect(() => {
    if (!status) {
      void Stores.RemoteAccess.loadStatus()
    }
    return () => {
      Stores.RemoteAccess.stopMagicLinkRotation()
    }
  }, [])

  // Wall-clock countdown until the magic link expires. Rerenders
  // every second; cheap.
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(t)
  }, [])
  const secondsLeft = useMemo(() => {
    if (!magicLink) return 0
    return Math.max(
      0,
      Math.floor((Date.parse(magicLink.expires_at) - now) / 1000),
    )
  }, [magicLink, now])

  const onCopy = async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text)
      message.success(`${label} copied`)
    } catch {
      message.error('Copy failed — your browser blocked clipboard access')
    }
  }

  // Tunnel-served session detection: the Tauri webview loads from
  // tauri://localhost or http://localhost:1420 (dev); a phone reaching
  // the same bundle over ngrok loads from `https://*.ngrok-free.app`
  // (or a custom domain). The Remote Access controls — start/stop
  // tunnel, set ngrok token, rotate admin password — are gated
  // server-side by the localhost-Host middleware AND only make sense
  // from the device that's actually being shared. Don't even render
  // the form on the phone; tell the user to go to the desktop.
  const isTunneledView = !['localhost', '127.0.0.1', '::1'].includes(
    window.location.hostname,
  )
  if (isTunneledView) {
    return (
      <SettingsPageContainer title="Remote Access">
        <Empty
          data-testid="desktop-remote-tunneled-empty"
          description={
            <div className="max-w-md mx-auto text-left">
              <Title level={5}>Open the desktop app</Title>
              <Paragraph type="secondary">
                This page configures the tunnel that's serving you right now —
                token, custom domain, password-auth toggle, start/stop. It can
                only be edited from the desktop app where the tunnel is hosted.
              </Paragraph>
              <Paragraph type="secondary" className="mb-0">
                If you need a new sign-in link, ask the desktop user to generate
                a fresh magic-link QR.
              </Paragraph>
            </div>
          }
        />
      </SettingsPageContainer>
    )
  }

  if (loading && !status) {
    return (
      <SettingsPageContainer title="Remote Access" subtitle="Loading…">
        <div />
      </SettingsPageContainer>
    )
  }

  if (!status) {
    return (
      <SettingsPageContainer title="Remote Access">
        <Empty
          data-testid="desktop-remote-load-error-empty"
          description={
            error
              ? `Unable to load remote-access status: ${error}`
              : 'Unable to load remote-access status'
          }
        >
          <Button
            data-testid="desktop-remote-retry-btn"
            icon={<RotateCw />}
            loading={loading}
            onClick={() => Stores.RemoteAccess.loadStatus()}
          >
            Retry
          </Button>
        </Empty>
      </SettingsPageContainer>
    )
  }

  const tunnelReady = status.auth_token_set
  const tunnelConnected = status.tunnel_state === 'connected'

  return (
    <SettingsPageContainer
      title="Remote Access"
      subtitle="Open this app to your phone or another browser through an ngrok tunnel."
    >
      {error && (
        <Alert
          data-testid="desktop-remote-error-alert"
          tone="error"
          title={error}
          onClose={() => Stores.RemoteAccess.loadStatus()}
          closeLabel="Close"
        />
      )}

      {/* 1. ngrok auth token */}
      <Card data-testid="desktop-remote-token-card" title="ngrok auth token">
        <div className="flex flex-col gap-1 mb-0">
          <label className="text-sm font-medium">Token</label>
          <div className="flex w-full gap-2">
            <PasswordInput
              data-testid="desktop-remote-token-input"
              className="flex-1"
              showLabel="Show token"
              hideLabel="Hide token"
              placeholder={
                status.auth_token_set
                  ? '••••••• (a token is saved)'
                  : 'Paste your ngrok auth token'
              }
              value={tokenDraft}
              onChange={e => setTokenDraft(e.target.value)}
              autoComplete="off"
            />
            <Button
              data-testid="desktop-remote-token-save-btn"
              disabled={!tokenDraft.trim() || saving}
              loading={saving}
              onClick={async () => {
                try {
                  await Stores.RemoteAccess.saveAuthToken(tokenDraft.trim())
                  setTokenDraft('')
                  message.success('ngrok auth token saved')
                } catch (e) {
                  message.error(
                    e instanceof Error ? e.message : 'Failed to save token',
                  )
                }
              }}
            >
              Save
            </Button>
          </div>
          <span className="text-xs text-muted-foreground">
            Paste your ngrok account's auth token. We'll keep it encrypted and
            never show it back to you.
          </span>
        </div>
        {status.auth_token_set && (
          <Text type="success">
            <CircleCheck className="inline size-4 align-text-bottom" /> Token
            saved
          </Text>
        )}
      </Card>

      {/* 2. Custom domain (optional) */}
      <Card
        data-testid="desktop-remote-domain-card"
        title="Custom domain (optional)"
      >
        <div className="flex flex-col gap-1 mb-0">
          <label className="text-sm font-medium">Domain</label>
          <div className="flex w-full gap-2">
            <Input
              // bespoke input-group: a draft input paired with an inline Save
              // button (imperative per-field save), not a uniform settings form field.
              data-standalone-control
              data-testid="desktop-remote-domain-input"
              className="flex-1"
              placeholder="my-app.ngrok.app (leave blank for auto-assigned)"
              value={domainDraft}
              onChange={e => setDomainDraft(e.target.value)}
            />
            <Button
              data-testid="desktop-remote-domain-save-btn"
              variant="outline"
              disabled={saving}
              loading={saving}
              onClick={async () => {
                const next = domainDraft.trim() || null
                try {
                  await Stores.RemoteAccess.saveDomain(next)
                  message.success('Domain saved')
                } catch (e) {
                  message.error(
                    e instanceof Error ? e.message : 'Failed to save domain',
                  )
                }
              }}
            >
              Save
            </Button>
          </div>
          <span className="text-xs text-muted-foreground">
            If your ngrok plan gives you a reserved subdomain, put it here so
            your URL stays the same every time. Leave it blank and ngrok will
            hand out a new URL on each restart.
          </span>
        </div>

        {/* 3. Auto-start (only visible with a fixed domain) */}
        {status.ngrok_domain && (
          <>
            <Separator className="my-3" />
            {/* Instant-apply toggle → the shadcn Field row idiom (label + description
                on the left, control on the right). */}
            <Field orientation="horizontal">
              <FieldContent>
                <FieldLabel htmlFor="desktop-remote-autostart-switch">
                  Auto-start tunnel on app launch
                </FieldLabel>
                <FieldDescription>
                  Bring your tunnel up automatically every time you start the
                  app. Only available with a fixed domain — without one, each
                  restart hands you a new URL and breaks any link you've already
                  shared.
                </FieldDescription>
              </FieldContent>
              <Switch
                id="desktop-remote-autostart-switch"
                data-testid="desktop-remote-autostart-switch"
                checked={status.auto_start_tunnel}
                loading={saving}
                onChange={async v => {
                  try {
                    await Stores.RemoteAccess.saveAutoStart(v)
                    message.success(
                      v ? 'Auto-start enabled' : 'Auto-start disabled',
                    )
                  } catch (e) {
                    message.error(
                      e instanceof Error
                        ? e.message
                        : 'Failed to update auto-start',
                    )
                  }
                }}
              />
            </Field>
          </>
        )}
      </Card>

      {/* 4. Password authentication (optional, OFF by default) */}
      <Card
        data-testid="desktop-remote-password-card"
        title="Password authentication"
      >
        <Paragraph type="secondary">
          By default, anyone you let in signs in by scanning the QR below. Turn
          this on if you'd also like to accept a password — handy when you want
          to share a long-lived link with someone who can't scan.
        </Paragraph>
        <PasswordAuthSection status={status} saving={saving} />
      </Card>

      {/* 5. Tunnel */}
      <Card
        data-testid="desktop-remote-tunnel-card"
        title={
          <Space>
            Tunnel
            <Tag
              data-testid="desktop-remote-tunnel-state-tag"
              tone={tunnelConnected ? 'success' : 'default'}
            >
              {status.tunnel_state}
            </Tag>
          </Space>
        }
      >
        {!tunnelReady && (
          <Alert
            data-testid="desktop-remote-no-token-alert"
            tone="warning"
            title="Add your ngrok auth token first"
            description="Save your token above, then come back here to start the tunnel."
          />
        )}
        {tunnelReady && !tunnelConnected && (
          <div className="flex flex-col gap-3">
            {status.auto_start_tunnel && status.last_error && (
              <Alert
                data-testid="desktop-remote-autostart-failed-alert"
                tone="error"
                title="Auto-start failed"
                description={
                  <>
                    The tunnel was supposed to start automatically at app launch
                    but ran into an error: <code>{status.last_error}</code>.
                    Click <strong>Start tunnel</strong> below to retry.
                  </>
                }
              />
            )}
            <Space>
              <Button
                data-testid="desktop-remote-start-tunnel-btn"
                loading={saving}
                onClick={() => Stores.RemoteAccess.startTunnel()}
              >
                Start tunnel
              </Button>
              {!status.auto_start_tunnel && status.last_error && (
                <Text type="danger">
                  <TriangleAlert className="inline size-4 align-text-bottom" />{' '}
                  {status.last_error}
                </Text>
              )}
            </Space>
          </div>
        )}
        {tunnelConnected && status.public_url && (
          <div className="flex flex-col gap-3">
            <Space wrap>
              <Button
                data-testid="desktop-remote-stop-tunnel-btn"
                variant="destructive"
                loading={saving}
                onClick={() => Stores.RemoteAccess.stopTunnel()}
              >
                Stop tunnel
              </Button>
              <Button
                data-testid="desktop-remote-new-code-btn"
                icon={<RotateCw />}
                loading={saving}
                onClick={() => Stores.RemoteAccess.rotateMagicLink()}
              >
                New code now
              </Button>
            </Space>

            {magicLink && (
              <div className="flex flex-col sm:flex-row gap-4 items-start">
                <Card
                  data-testid="desktop-remote-qr-card"
                  size="sm"
                  className="flex-shrink-0"
                >
                  <QRCodeSVG value={magicLink.url} size={200} />
                </Card>
                <div className="flex-1 flex flex-col gap-2">
                  <Text strong>Scan with your phone to sign in</Text>
                  <Text type="secondary">
                    Each code works once and expires in{' '}
                    {Math.floor(secondsLeft / 60)}:
                    {String(secondsLeft % 60).padStart(2, '0')}. A fresh one
                    appears every 4 minutes.
                  </Text>
                  <div className="flex w-full gap-2">
                    <Input
                      data-standalone-control
                      data-testid="desktop-remote-magic-link-input"
                      className="flex-1"
                      readOnly
                      value={magicLink.url}
                    />
                    <Tooltip title="Copy">
                      <Button
                        data-testid="desktop-remote-copy-magic-link-btn"
                        aria-label="Copy magic link"
                        icon={<Copy />}
                        onClick={() => onCopy(magicLink.url, 'Magic link')}
                      />
                    </Tooltip>
                  </div>
                  {status.password_auth_enabled ? (
                    <div className="flex flex-col gap-1">
                      <Text type="secondary" className="text-xs">
                        Or send this plain URL — whoever opens it will be asked
                        for your password:
                      </Text>
                      <div className="flex w-full gap-2">
                        <Input
                          data-standalone-control
                          data-testid="desktop-remote-bare-url-input"
                          className="flex-1"
                          readOnly
                          value={status.public_url}
                        />
                        <Tooltip title="Copy">
                          <Button
                            data-testid="desktop-remote-copy-bare-url-btn"
                            aria-label="Copy bare URL"
                            icon={<Copy />}
                            onClick={() =>
                              onCopy(status.public_url!, 'Bare URL')
                            }
                          />
                        </Tooltip>
                      </div>
                    </div>
                  ) : (
                    <Text type="secondary" className="text-xs">
                      You haven't turned on password login, so send a fresh
                      magic link from here every time you add a new device.
                    </Text>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </Card>
    </SettingsPageContainer>
  )
}

const changePasswordSchema = z
  .object({
    new_password: z.string().min(1, 'Required').min(8, 'At least 8 characters'),
    confirm: z.string().min(1, 'Required'),
  })
  .refine(d => d.new_password === d.confirm, {
    message: 'Passwords do not match',
    path: ['confirm'],
  })
type ChangePasswordValues = z.infer<typeof changePasswordSchema>

/**
 * Inline password-auth toggle + (when first enabling) the
 * change-password form. Keeps both concerns in one card so the
 * user never has to navigate away mid-setup.
 */
function PasswordAuthSection({
  status,
  saving,
}: {
  status: { password_rotated: boolean; password_auth_enabled: boolean }
  saving: boolean
}) {
  const [showChangePassword, setShowChangePassword] = useState(false)
  const form = useForm<ChangePasswordValues>({
    resolver: zodResolver(changePasswordSchema),
    defaultValues: { new_password: '', confirm: '' },
  })
  const [submitting, setSubmitting] = useState(false)

  // When the toggle is flipped ON for the first time (password not
  // yet rotated), show the inline change-password form. Once the
  // user sets a new password, the toggle can actually flip on
  // server-side.
  const needsRotationToEnable = !status.password_rotated

  const submitChangePassword = async (v: ChangePasswordValues) => {
    if (v.new_password !== v.confirm) {
      message.error('Passwords do not match')
      return
    }
    setSubmitting(true)
    try {
      // Routed through the store (which calls the desktop-only
      // /api/remote-access/admin-password endpoint, gated by the
      // localhost-Host middleware — the desktop user's physical
      // presence is the auth proof).
      await Stores.RemoteAccess.setAdminPassword(v.new_password)
      message.success('Password set')
      // Now safe to flip the toggle on.
      await Stores.RemoteAccess.setPasswordAuthEnabled(true)
      setShowChangePassword(false)
      form.reset()
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to set password')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="mb-0">
      {/* Instant-apply toggle → the shadcn Field row idiom. */}
      <Field orientation="horizontal">
        <FieldContent>
          <FieldLabel htmlFor="desktop-remote-password-enable-switch">
            Enable password authentication
          </FieldLabel>
          <FieldDescription>
            Off by default — only the magic-link QR works for new devices.
          </FieldDescription>
        </FieldContent>
        <Switch
          id="desktop-remote-password-enable-switch"
          data-testid="desktop-remote-password-enable-switch"
          checked={status.password_auth_enabled}
          loading={saving}
          onChange={async v => {
            if (v && needsRotationToEnable) {
              setShowChangePassword(true)
              return
            }
            try {
              await Stores.RemoteAccess.setPasswordAuthEnabled(v)
              message.success(
                v
                  ? 'Password authentication enabled'
                  : 'Password authentication disabled',
              )
            } catch (e) {
              message.error(
                e instanceof Error
                  ? e.message
                  : 'Failed to update password authentication',
              )
            }
          }}
        />
      </Field>

      {status.password_auth_enabled && status.password_rotated && (
        <Button
          data-testid="desktop-remote-change-password-toggle-btn"
          variant="link"
          className="p-0"
          onClick={() => setShowChangePassword(v => !v)}
        >
          {showChangePassword ? 'Hide' : 'Change password'}
        </Button>
      )}

      {showChangePassword && (
        <div className="mt-3 p-3 rounded border border-border bg-muted/40">
          <Title level={5} className="mt-0">
            {needsRotationToEnable
              ? 'Set a strong admin password'
              : 'Change admin password'}
          </Title>
          {needsRotationToEnable && (
            <Paragraph type="secondary">
              Your password is still the published default (
              <code>desktop-auto-login</code>) — anyone with your tunnel URL
              could use it. Pick something only you know. You don't need to type
              the current one; this form only works from your own machine.
            </Paragraph>
          )}
          <Form
            data-testid="desktop-remote-change-password-form"
            form={form}
            onSubmit={submitChangePassword}
            layout="vertical"
          >
            <FormField
              name="new_password"
              label="New password"
              description="At least 8 characters. The longer and more random, the better — this is what protects your app once it's reachable from the internet."
            >
              <PasswordInput
                data-testid="desktop-remote-new-password-input"
                autoComplete="new-password"
                showLabel="Show password"
                hideLabel="Hide password"
              />
            </FormField>
            <FormField name="confirm" label="Confirm new password">
              <PasswordInput
                data-testid="desktop-remote-confirm-password-input"
                autoComplete="new-password"
                showLabel="Show password"
                hideLabel="Hide password"
              />
            </FormField>
            <Space>
              <Button
                data-testid="desktop-remote-save-password-btn"
                type="submit"
                loading={submitting}
              >
                Save password{needsRotationToEnable && ' and enable'}
              </Button>
              <Button
                data-testid="desktop-remote-cancel-password-btn"
                type="button"
                variant="outline"
                onClick={() => setShowChangePassword(false)}
              >
                Cancel
              </Button>
            </Space>
          </Form>
        </div>
      )}
    </div>
  )
}
