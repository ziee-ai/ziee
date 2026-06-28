import { useEffect, useMemo, useState } from 'react'
import { z } from 'zod'
import {
  Alert,
  Button,
  Flex,
  Form,
  FormField,
  useForm,
  zodResolver,
  Input,
  PasswordInput,
  Spin,
  Switch,
  Text,
  Title,
  Paragraph,
  message,
} from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import {
  Permissions,
  type AuthProviderResponse,
  type TestProviderResponse,
} from '@/api-client/types'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions/Can'
import { usePermission } from '@/core/permissions/usePermission'
import type { ProviderTemplate } from '../types'

/**
 * Drawer mode: either create-from-template (no `existing` row, has
 * `template`) or edit-existing (has `existing` row, no `template`).
 */
export interface EditDrawerProps {
  open: boolean
  template?: ProviderTemplate
  existing?: AuthProviderResponse
  onClose: () => void
}

interface FormShape {
  name: string
  enabled: boolean
  config: Record<string, any>
}

const SECRET_FIELDS = ['client_secret'] as const

// Zod schema for form validation. Config fields vary by provider type
// so they are all optional here; required-field enforcement is visual
// (required prop on FormField) and enforced by the backend on save.
// The MS-tenant cross-field rule is encoded in superRefine.
const formSchema = z.object({
  name: z
    .string()
    .min(1, 'Provider name required')
    .regex(
      /^[a-z0-9-]+$/,
      'Lowercase letters, digits, and hyphens only (appears in URLs)',
    ),
  enabled: z.boolean(),
  config: z
    .object({
      client_id: z.string().optional(),
      client_secret: z.string().optional(),
      issuer_url: z.string().optional(),
      scopes: z.union([z.string(), z.array(z.string())]).optional(),
      allowed_tenant_ids: z.union([z.string(), z.array(z.string())]).optional(),
      display_name: z.string().optional(),
      authorization_url: z.string().optional(),
      token_url: z.string().optional(),
      userinfo_url: z.string().optional(),
      team_id: z.string().optional(),
      services_id: z.string().optional(),
      key_id: z.string().optional(),
      private_key_path: z
        .string()
        .optional()
        .refine(
          (v) => !v || /^\/.+/.test(v),
          'Use an absolute filesystem path (must start with `/`)',
        ),
    })
    .catchall(z.any())
    .superRefine((config, ctx) => {
      const issuer = (config.issuer_url as string | undefined) ?? ''
      // Microsoft's `common` / templated-issuer flow refuses
      // to operate without an allowlist (backend enforces;
      // surface earlier in the UI).
      const needsAllowlist =
        /\/(common|organizations|consumers)(\/|$)/.test(issuer) ||
        issuer.includes('{tenantid}')
      const tenantIds = config.allowed_tenant_ids
      if (
        needsAllowlist &&
        (typeof tenantIds === 'string'
          ? tenantIds.trim() === ''
          : !Array.isArray(tenantIds) || tenantIds.length === 0)
      ) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          message:
            'At least one tenant ID is required for the Microsoft `common` endpoint',
          path: ['allowed_tenant_ids'],
        })
      }
    }),
})

/**
 * Per-provider-type field renderer. OIDC + OAuth2 are similar
 * (client_id/secret + endpoint URLs); Apple is entirely different
 * (team_id/services_id/key_id/.p8 path).
 *
 * Fields are rendered with `disabled={!canManage}` so a user with
 * read-only permission sees the config but can't change it. The
 * submit button is wrapped in `<Can>` so they don't see it at all.
 */
export function AuthProviderEditDrawer({
  open,
  template,
  existing,
  onClose,
}: EditDrawerProps) {
  const form = useForm<FormShape>({
    resolver: zodResolver(formSchema),
    defaultValues: { name: '', enabled: false, config: {} },
  })
  const { saving, error } = Stores.AuthProvidersAdmin
  const canManage = usePermission(Permissions.AuthProvidersManage)
  const [testing, setTesting] = useState(false)
  // Distinct from `testing` (the manual "Test config" button's
  // spinner) so the Switch's spinner doesn't light up while the user
  // is using the diagnostic button, and vice versa.
  const [togglingEnable, setTogglingEnable] = useState(false)
  const [testResult, setTestResult] = useState<TestProviderResponse | null>(
    null,
  )

  // Resolve the provider type for this drawer instance.
  const providerType = useMemo<string>(
    () => template?.provider_type ?? existing?.provider_type ?? 'oidc',
    [template, existing],
  )

  useEffect(() => {
    if (!open) return
    setTestResult(null)
    if (template) {
      // Auto-fill `name` from the template key for Google / Microsoft
      // / Apple. The migration 47 pre-seed already created rows with
      // those names, so the Add menu disables those entries (see
      // AddProviderMenu.existingNames); this branch only fires for
      // generic templates where `key` doesn't collide.
      const defaultName = templateDefaultName(template)
      form.reset({
        name: defaultName,
        // Safer default: rows are created disabled. Admin verifies
        // with Test config + toggles on when ready.
        enabled: false,
        config: template.defaultConfig,
      })
    } else if (existing) {
      form.reset({
        name: existing.name,
        enabled: existing.enabled,
        config: existing.config ?? {},
      })
    }
  }, [open, template, existing, form])

  const onTestConfig = async () => {
    setTesting(true)
    setTestResult(null)
    try {
      const valid = await form.trigger()
      if (!valid) return
      const values = form.getValues() as FormShape
      const normalized = normalizeConfig(values.config, providerType)
      const res = await Stores.AuthProvidersAdmin.testConfig({
        name: values.name.trim(),
        provider_type: providerType,
        enabled: false,
        config: normalized,
      })
      setTestResult(res)
    } finally {
      setTesting(false)
    }
  }

  // Intercept the enable Switch. Mirrors `LlmRepositoryDrawer.tsx`:
  //  - OFF in either mode → flip the form value, no probe.
  //  - ON in CREATE mode → run stateless `testConfig`; snap back on
  //    failure so the admin can't ship a broken row to the backend.
  //  - ON in EDIT mode → save the full form (forcing enabled=true) so
  //    the backend's `enforce_on_update_transition` probes the
  //    persisted config; on 400 the store's auto_disabled emit + the
  //    catch here snap the Switch back.
  const handleEnabledToggle = async (next: boolean) => {
    // Guard against rapid toggling while a probe is in flight.
    if (togglingEnable) return

    if (!next) {
      form.setValue('enabled', false)
      return
    }

    if (template) {
      // CREATE mode: stateless probe.
      setTogglingEnable(true)
      try {
        const valid = await form.trigger()
        if (!valid) {
          form.setValue('enabled', false)
          return
        }
        const values = form.getValues() as FormShape
        const normalized = normalizeConfig(values.config, providerType)
        const res = await Stores.AuthProvidersAdmin.testConfig({
          name: values.name.trim(),
          provider_type: providerType,
          enabled: false,
          config: normalized,
        })
        if (!res.ok) {
          form.setValue('enabled', false)
          // Clear any stale success Alert from a prior "Test config"
          // run so the operator sees the fresh failure, not a stale
          // green success.
          setTestResult(res)
          message.error(`Cannot enable: ${res.message}`, { duration: 8000 })
          return
        }
        form.setValue('enabled', true)
        setTestResult(res)
      } finally {
        setTogglingEnable(false)
      }
      return
    }

    // EDIT mode: persist the full form with enabled=true and let the
    // backend's enforce_on_update_transition do the probe.
    if (!existing) return
    setTogglingEnable(true)
    try {
      const valid = await form.trigger()
      if (!valid) {
        form.setValue('enabled', false)
        return
      }
      const values = form.getValues() as FormShape
      const normalized = normalizeConfig(values.config, providerType)
      form.setValue('enabled', true)
      try {
        await Stores.AuthProvidersAdmin.updateProvider(existing.id, {
          name: values.name.trim(),
          enabled: true,
          config: normalized,
        })
        message.success('Provider enabled — connection test passed.')
      } catch (e: any) {
        // The store emits auto_disabled on the 400 enable-failed code,
        // which triggers a list reload + the canonical row state
        // (enabled=false). Snap the local form value back so the
        // Switch reflects reality.
        form.setValue('enabled', false)
        const reason =
          typeof e?.message === 'string'
            ? e.message
            : 'Connection probe failed; provider remains disabled.'
        message.error(`Failed to enable: ${reason}`, { duration: 8000 })
      }
    } finally {
      setTogglingEnable(false)
    }
  }

  const onValidSubmit = async (values: FormShape) => {
    // Normalize: scopes can be entered as comma-separated string.
    const normalized = normalizeConfig(values.config, providerType)
    try {
      if (template) {
        const provider = await Stores.AuthProvidersAdmin.createProvider({
          name: values.name.trim(),
          provider_type: providerType,
          enabled: values.enabled,
          config: normalized,
        })
        // The store surfaces a connection_warning via the
        // auto_disabled event already; we just tell the admin if the
        // row landed disabled vs enabled.
        if (values.enabled && !provider.enabled) {
          message.error(
            `Created ${values.name.trim()} but disabled — connection probe failed. Fix the config and re-enable.`,
            { duration: 8000 },
          )
        } else {
          message.success(`Created ${values.name.trim()}`)
        }
      } else if (existing) {
        await Stores.AuthProvidersAdmin.updateProvider(existing.id, {
          name: values.name.trim(),
          enabled: values.enabled,
          config: normalized,
        })
        message.success(`Saved ${existing.name}`)
      }
      onClose()
    } catch (e: any) {
      // 400 AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK lands here. The
      // store's auto_disabled emit already triggered a reload + Switch
      // snap-back; surface the reason here for the admin to read.
      const reason = e?.message
      if (typeof reason === 'string' && reason.length > 0) {
        message.error(reason, { duration: 8000 })
        // Snap the drawer's form value back to disabled so the visual
        // state matches the canonical row state.
        form.setValue('enabled', false)
      }
    }
  }
  const handleSave = form.handleSubmit(onValidSubmit)

  const titleText = existing
    ? `Edit ${existing.name}`
    : template
      ? `Add ${template.label}`
      : 'New provider'

  return (
    <Drawer
      title={titleText}
      open={open}
      onClose={onClose}
      size={600}
      maskClosable={false}
      destroyOnHidden
      footer={
        // Cancel/Close → Save, right-aligned. Matches the dominant
        // convention from 08-cross-cutting-consistency I-1 (every
        // non-user-module drawer uses justify-end + Cancel-then-Submit).
        // Read-only users get a "Close" button instead of "Cancel"
        // because there's nothing to cancel.
        <Flex className="justify-end gap-2">
          <Button variant="outline" data-testid="authprov-drawer-cancel-button" onClick={onClose} disabled={saving}>
            {canManage ? 'Cancel' : 'Close'}
          </Button>
          <Can permission={Permissions.AuthProvidersManage}>
            {/* Footer button lives OUTSIDE the form (project Drawer
                convention), so htmlType="submit" wouldn't work
                here — Enter-to-submit is handled by Form's
                `onSubmit={handleSave}` instead. */}
            <Button data-testid="authprov-drawer-save-button" loading={saving} onClick={handleSave}>
              {existing ? 'Save' : 'Create'}
            </Button>
          </Can>
        </Flex>
      }
    >
      {/* The project's <Drawer> wrapper applies `flex w-full` to its
          children container, so multiple top-level children get laid
          out as a flex ROW (alert sits left of the form, breaking
          the layout — see visual smoke test). Wrap in a single column
          container so everything stacks vertically inside the
          drawer body. */}
      <div className="flex flex-col w-full">
        {error && (
          <Alert tone="error" data-testid="authprov-drawer-error-alert" title={error} className="mb-4" />
        )}
        {testing && (
          <div className="text-center py-3 mb-4">
            <Spin label="Testing" /> <Text type="secondary">Testing…</Text>
          </div>
        )}
        {testResult && !testing && (
          <Alert
            data-testid="authprov-drawer-testresult-alert"
            tone={testResult.ok ? 'success' : 'warning'}
            title={testResult.ok ? 'Configuration OK' : 'Configuration issues'}
            description={testResult.message}
            onClose={() => setTestResult(null)}
            closeLabel="Close"
            className="mb-4"
          />
        )}
        <Form form={form} data-testid="authprov-drawer-form" layout="vertical" disabled={!canManage} onSubmit={onValidSubmit}>
          <FormField
            name="name"
            label="Name (URL slug)"
            required
          >
            <Input
              data-testid="authprov-name-input"
              placeholder="e.g. google, microsoft-corp, apple"
              disabled={!!existing}
            />
          </FormField>
          <FormField
            name="enabled"
            label="Enabled"
            valuePropName="checked"
            description={
              template
                ? 'Toggling on runs a quick connection probe; if it fails the Switch reverts. Save with this on to create the provider enabled.'
                : 'Toggling on commits the form and runs a connection probe server-side; the Switch reverts if the probe fails.'
            }
          >
            <Switch data-testid="authprov-enabled-switch" loading={togglingEnable} onChange={handleEnabledToggle} />
          </FormField>

          <Title level={5} className="mt-2">
            Configuration
          </Title>
          {providerType === 'apple' ? (
            <AppleFields />
          ) : providerType === 'oauth2' ? (
            <OAuth2Fields />
          ) : (
            <OidcFields />
          )}

          {existing && (
            <Paragraph type="secondary" className="mt-3 text-xs">
              Leave <code>client_secret</code> empty to keep the existing value.
            </Paragraph>
          )}
          <Can permission={Permissions.AuthProvidersManage}>
            <Flex className="mt-4">
              <Button data-testid="authprov-test-config-button" loading={testing} onClick={onTestConfig}>
                Test config
              </Button>
            </Flex>
          </Can>
        </Form>
      </div>
    </Drawer>
  )
}

function OidcFields() {
  return (
    <>
      <FormField
        name="config.client_id"
        label="Client ID"
        required
      >
        <Input data-testid="authprov-oidc-client-id-input" />
      </FormField>
      <FormField name="config.client_secret" label="Client secret">
        <PasswordInput
          data-testid="authprov-oidc-client-secret-input"
          placeholder="••••••  (leave empty to keep existing)"
          autoComplete="new-password"
          showLabel="Show"
          hideLabel="Hide"
        />
      </FormField>
      <FormField
        name="config.issuer_url"
        label="Issuer URL"
        required
      >
        <Input data-testid="authprov-oidc-issuer-url-input" placeholder="https://accounts.google.com" />
      </FormField>
      <FormField
        name="config.scopes"
        label="Scopes"
      >
        <Input data-testid="authprov-oidc-scopes-input" placeholder="openid email profile" />
      </FormField>
      <FormField
        name="config.allowed_tenant_ids"
        label="Allowed tenant IDs (Microsoft only)"
        description="Comma-separated list of tenant IDs (the `tid` claim). REQUIRED when issuer is the Microsoft `common` endpoint."
      >
        <Input data-testid="authprov-oidc-tenant-ids-input" placeholder="comma-separated UUIDs (leave blank for non-MS)" />
      </FormField>
      <FormField name="config.display_name" label="Button label">
        <Input data-testid="authprov-oidc-display-name-input" placeholder="Sign in with X" />
      </FormField>
    </>
  )
}

function OAuth2Fields() {
  return (
    <>
      <FormField
        name="config.client_id"
        label="Client ID"
        required
      >
        <Input data-testid="authprov-oauth2-client-id-input" />
      </FormField>
      <FormField name="config.client_secret" label="Client secret">
        <PasswordInput
          data-testid="authprov-oauth2-client-secret-input"
          placeholder="••••••  (leave empty to keep existing)"
          autoComplete="new-password"
          showLabel="Show"
          hideLabel="Hide"
        />
      </FormField>
      <FormField
        name="config.authorization_url"
        label="Authorization URL"
        required
      >
        <Input data-testid="authprov-oauth2-authorization-url-input" />
      </FormField>
      <FormField
        name="config.token_url"
        label="Token URL"
        required
      >
        <Input data-testid="authprov-oauth2-token-url-input" />
      </FormField>
      <FormField
        name="config.userinfo_url"
        label="UserInfo URL"
      >
        <Input data-testid="authprov-oauth2-userinfo-url-input" />
      </FormField>
      <FormField
        name="config.scopes"
        label="Scopes"
      >
        <Input data-testid="authprov-oauth2-scopes-input" placeholder="email profile" />
      </FormField>
      <FormField name="config.display_name" label="Button label">
        <Input data-testid="authprov-oauth2-display-name-input" placeholder="Sign in with X" />
      </FormField>
    </>
  )
}

function AppleFields() {
  return (
    <>
      <FormField
        name="config.team_id"
        label="Team ID"
        required
      >
        <Input data-testid="authprov-apple-team-id-input" placeholder="10-char Apple Developer Team ID" />
      </FormField>
      <FormField
        name="config.services_id"
        label="Services ID"
        required
      >
        <Input data-testid="authprov-apple-services-id-input" placeholder="The Apple-issued client identifier (Services ID)" />
      </FormField>
      <FormField
        name="config.key_id"
        label="Key ID"
        required
      >
        <Input data-testid="authprov-apple-key-id-input" placeholder="10-char Key ID for the .p8 file" />
      </FormField>
      <FormField
        name="config.private_key_path"
        label="Private key path on disk"
        required
        description="Filesystem path to the AuthKey_<KEY_ID>.p8 file. The file itself stays on disk with proper permissions — it is not uploaded through the UI."
      >
        <Input data-testid="authprov-apple-private-key-path-input" placeholder="/var/lib/ziee/apple/AuthKey_XXXXXXXXXX.p8" />
      </FormField>
      <FormField
        name="config.scopes"
        label="Scopes"
      >
        <Input data-testid="authprov-apple-scopes-input" placeholder="name email" />
      </FormField>
    </>
  )
}

function normalizeScopeArray(value: any): string[] {
  if (Array.isArray(value)) {
    return value
      .map(v => (typeof v === 'string' ? v.trim() : ''))
      .filter(Boolean)
  }
  if (typeof value === 'string') {
    return value
      .split(/[\s,]+/)
      .map(s => s.trim())
      .filter(Boolean)
  }
  return []
}

/// Auto-fill the `name` field for Google / Microsoft / Apple
/// templates so the row matches the migration-47 pre-seed naming.
/// Other templates (generic OIDC/OAuth2) leave it blank for the
/// admin to fill.
function templateDefaultName(t: ProviderTemplate): string {
  if (t.key === 'google' || t.key === 'microsoft' || t.key === 'apple') {
    return t.key
  }
  return ''
}

function normalizeConfig(
  config: Record<string, any>,
  _providerType: string,
): Record<string, any> {
  const out = { ...config }
  // Drop empty secret-ish fields so the backend "leave unchanged"
  // behavior kicks in cleanly.
  for (const k of SECRET_FIELDS) {
    if (typeof out[k] === 'string' && out[k].trim() === '') {
      delete out[k]
    }
  }
  // Normalize scopes and allowed_tenant_ids from comma/space-separated
  // string (as entered in the Input) to string[] for the backend.
  for (const k of ['scopes', 'allowed_tenant_ids'] as const) {
    if (out[k] !== undefined) {
      out[k] = normalizeScopeArray(out[k])
    }
  }
  // Drop empty allowed_tenant_ids array — server treats absent ==
  // None, which means "no tenant restriction".
  if (Array.isArray(out.allowed_tenant_ids) && out.allowed_tenant_ids.length === 0) {
    delete out.allowed_tenant_ids
  }
  return out
}
