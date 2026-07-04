import { useEffect } from 'react'
import {
  Alert,
  Card,
  Form,
  FormField,
  InputNumber,
  Spin,
  message,
  useForm,
} from '@/components/ui'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

type FormValues = {
  access_token_expiry_hours: number
  refresh_token_expiry_days: number
}

/**
 * Admin "Sessions" settings page — the deployment-wide JWT lifetimes:
 * how long an access token lives (clients silently refresh before it
 * expires) and the max session length (how long a session survives with
 * no activity before the user must sign in again). Changes apply to
 * tokens minted from that moment on; existing tokens keep their exp.
 */
export function SessionSettingsPage() {
  const { settings, loading, saving, error } = Stores.SessionSettings
  const canManage = usePermission(Permissions.SessionSettingsManage)

  const form = useForm<FormValues>()

  // Re-seed from the store ONLY when the form has no unsaved edits, so a
  // sync-driven refetch doesn't clobber in-progress values.
  useEffect(() => {
    if (settings && !form.formState.isDirty) {
      form.reset({
        access_token_expiry_hours: settings.access_token_expiry_hours,
        refresh_token_expiry_days: settings.refresh_token_expiry_days,
      })
    }
  }, [settings, form])

  const onSubmit = async (v: FormValues) => {
    try {
      await Stores.SessionSettings.update({
        access_token_expiry_hours: v.access_token_expiry_hours,
        refresh_token_expiry_days: v.refresh_token_expiry_days,
      })
      form.reset(v) // saved → allow the next store update to re-seed
      message.success('Session settings saved')
    } catch (e: unknown) {
      message.error(
        e instanceof Error ? e.message : 'Failed to save session settings',
      )
    }
  }

  const subtitle =
    'How long sign-ins last. Active sessions silently renew their access ' +
    'token before it expires; the session length bounds how long an idle ' +
    'session survives before the user must sign in again.'

  if (loading && !settings) {
    return (
      <SettingsPageContainer title="Sessions" subtitle={subtitle}>
        <div className="flex justify-center py-12">
          <Spin size="lg" label="Loading session settings" />
        </div>
      </SettingsPageContainer>
    )
  }

  return (
    <SettingsPageContainer title="Sessions" subtitle={subtitle}>
      {error && (
        <Alert
          data-testid="session-settings-error-alert"
          tone="error"
          title="Failed to load session settings"
          description={error}
          className="mb-3"
        />
      )}
      <Card
        data-testid="session-settings-card"
        title="Token lifetimes"
        footer={
          <SettingsFormActions
            onSave={form.handleSubmit(onSubmit)}
            onCancel={() => form.reset()}
            saving={saving}
            saveDisabled={!canManage || !form.formState.isDirty}
            cancelDisabled={!canManage}
            saveTestid="session-settings-save"
            cancelTestid="session-settings-cancel"
          />
        }
      >
        {!canManage && (
          <Alert
            data-testid="session-settings-readonly-alert"
            tone="info"
            title="Read-only view"
            description="You can view session settings but not change them."
            className="mb-3"
          />
        )}

        <Form
          data-testid="session-settings-form"
          form={form}
          layout="horizontal"
          disabled={!canManage}
          onSubmit={onSubmit}
        >
          <FormField
            name="access_token_expiry_hours"
            label="Access token lifetime"
            description="Shorter is safer — a deactivated account is cut off at the next renewal. Signed-in clients renew automatically, so users don't notice."
          >
            <InputNumber
              data-testid="session-settings-access-hours"
              min={1}
              max={8760}
              suffix="hours"
              className="w-full"
            />
          </FormField>
          <FormField
            name="refresh_token_expiry_days"
            label="Session length"
            description="How long a session survives with no activity before the user must sign in again. Active sessions roll forward on every renewal."
          >
            <InputNumber
              data-testid="session-settings-session-days"
              min={1}
              max={3650}
              suffix="days"
              className="w-full"
            />
          </FormField>
        </Form>
      </Card>
    </SettingsPageContainer>
  )
}
