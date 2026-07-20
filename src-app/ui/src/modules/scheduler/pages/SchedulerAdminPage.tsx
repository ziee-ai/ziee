import { useEffect, useState } from 'react'

import { Permissions } from '@/api-client/types'
import {
  Alert,
  Button,
  Card,
  Flex,
  InputNumber,
  Spin,
  message,
} from '@ziee/kit'
import {
  Field,
  FieldContent,
  FieldGroup,
  FieldTitle,
} from '@ziee/kit/shadcn/field'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

interface Form {
  max_active_tasks_per_user: number
  min_interval_seconds: number
  max_horizon_days: number
  max_consecutive_failures: number
  notification_retention_days: number
}

export function SchedulerAdminPage() {
  const { settings, loading, saving, error } = Stores.SchedulerAdmin
  const canManage = usePermission(Permissions.SchedulerAdminManage)
  const [f, setF] = useState<Form | null>(null)

  useEffect(() => {
    void Stores.SchedulerAdmin.loadSettings()
  }, [])
  useEffect(() => {
    if (settings)
      setF({
        max_active_tasks_per_user: settings.max_active_tasks_per_user,
        min_interval_seconds: settings.min_interval_seconds,
        max_horizon_days: settings.max_horizon_days,
        max_consecutive_failures: settings.max_consecutive_failures,
        notification_retention_days: settings.notification_retention_days,
      })
  }, [settings])

  const save = async () => {
    if (!f) return
    try {
      await Stores.SchedulerAdmin.updateSettings(f)
      message.success('Scheduler settings saved')
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to save')
    }
  }

  if (loading && !settings) {
    return (
      <SettingsPageContainer
        title="Scheduler"
        subtitle="Deployment-wide scheduling limits."
      >
        <Flex className="justify-center py-12">
          <Spin size="lg" label="Loading scheduler settings" />
        </Flex>
      </SettingsPageContainer>
    )
  }

  return (
    <SettingsPageContainer
      title="Scheduler"
      subtitle="Deployment-wide scheduling limits."
      data-testid="scheduler-admin-page"
    >
      <Card data-testid="scheduler-admin-card" title="Limits">
        {!canManage && (
          <Alert
            tone="info"
            title="Read-only view"
            description="You need scheduler admin rights to change these."
            data-testid="scheduler-admin-readonly"
            className="mb-3"
          />
        )}
        {error && (
          <Alert
            tone="error"
            title="Error"
            description={error}
            className="mb-3"
            data-testid="scheduler-admin-error"
          />
        )}
        {f && (
          <Flex className="flex-col gap-3">
            <FieldGroup>
              <Field orientation="horizontal">
                <FieldContent>
                  <FieldTitle>Max active tasks per user</FieldTitle>
                </FieldContent>
                <InputNumber
                  data-testid="scheduler-max-active"
                  aria-label="Max active tasks per user"
                  min={1}
                  max={1000}
                  value={f.max_active_tasks_per_user}
                  disabled={!canManage}
                  onChange={v =>
                    setF({ ...f, max_active_tasks_per_user: Number(v ?? 1) })
                  }
                />
              </Field>
              <Field orientation="horizontal">
                <FieldContent>
                  <FieldTitle>Minimum interval (seconds)</FieldTitle>
                </FieldContent>
                <InputNumber
                  data-testid="scheduler-min-interval"
                  aria-label="Minimum interval (seconds)"
                  min={60}
                  max={86400}
                  value={f.min_interval_seconds}
                  disabled={!canManage}
                  onChange={v =>
                    setF({ ...f, min_interval_seconds: Number(v ?? 300) })
                  }
                />
              </Field>
              <Field orientation="horizontal">
                <FieldContent>
                  <FieldTitle>Self-paced loop horizon (days)</FieldTitle>
                </FieldContent>
                <InputNumber
                  data-testid="scheduler-max-horizon"
                  aria-label="Self-paced loop horizon (days)"
                  min={1}
                  max={365}
                  value={f.max_horizon_days}
                  disabled={!canManage}
                  onChange={v =>
                    setF({ ...f, max_horizon_days: Number(v ?? 7) })
                  }
                />
              </Field>
              <Field orientation="horizontal">
                <FieldContent>
                  <FieldTitle>
                    Auto-pause after N consecutive failures
                  </FieldTitle>
                </FieldContent>
                <InputNumber
                  data-testid="scheduler-max-failures"
                  aria-label="Auto-pause after N consecutive failures"
                  min={1}
                  max={100}
                  value={f.max_consecutive_failures}
                  disabled={!canManage}
                  onChange={v =>
                    setF({ ...f, max_consecutive_failures: Number(v ?? 5) })
                  }
                />
              </Field>
              <Field orientation="horizontal">
                <FieldContent>
                  <FieldTitle>
                    Notification retention (days, 0 = forever)
                  </FieldTitle>
                </FieldContent>
                <InputNumber
                  data-testid="scheduler-retention"
                  aria-label="Notification retention (days)"
                  min={0}
                  max={3650}
                  value={f.notification_retention_days}
                  disabled={!canManage}
                  onChange={v =>
                    setF({ ...f, notification_retention_days: Number(v ?? 30) })
                  }
                />
              </Field>
            </FieldGroup>
            {canManage && (
              <Flex className="justify-end">
                <Button
                  data-testid="scheduler-admin-save"
                  onClick={save}
                  loading={saving}
                >
                  Save
                </Button>
              </Flex>
            )}
          </Flex>
        )}
      </Card>
    </SettingsPageContainer>
  )
}
