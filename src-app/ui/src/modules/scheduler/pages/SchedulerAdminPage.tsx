import { useEffect, useState } from 'react'

import { Permissions } from '@/api-client/types'
import {
  Alert,
  Button,
  Card,
  Flex,
  InputNumber,
  Spin,
  Text,
  message,
} from '@/components/ui'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

interface Form {
  max_active_tasks_per_user: number
  min_interval_seconds: number
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
            <Flex className="items-center justify-between gap-2">
              <Text className="text-sm">Max active tasks per user</Text>
              <InputNumber
                data-testid="scheduler-max-active"
                min={1}
                max={1000}
                value={f.max_active_tasks_per_user}
                disabled={!canManage}
                onChange={v =>
                  setF({ ...f, max_active_tasks_per_user: Number(v ?? 1) })
                }
              />
            </Flex>
            <Flex className="items-center justify-between gap-2">
              <Text className="text-sm">Minimum interval (seconds)</Text>
              <InputNumber
                data-testid="scheduler-min-interval"
                min={60}
                max={86400}
                value={f.min_interval_seconds}
                disabled={!canManage}
                onChange={v =>
                  setF({ ...f, min_interval_seconds: Number(v ?? 300) })
                }
              />
            </Flex>
            <Flex className="items-center justify-between gap-2">
              <Text className="text-sm">
                Auto-pause after N consecutive failures
              </Text>
              <InputNumber
                data-testid="scheduler-max-failures"
                min={1}
                max={100}
                value={f.max_consecutive_failures}
                disabled={!canManage}
                onChange={v =>
                  setF({ ...f, max_consecutive_failures: Number(v ?? 5) })
                }
              />
            </Flex>
            <Flex className="items-center justify-between gap-2">
              <Text className="text-sm">
                Notification retention (days, 0 = forever)
              </Text>
              <InputNumber
                data-testid="scheduler-retention"
                min={0}
                max={3650}
                value={f.notification_retention_days}
                disabled={!canManage}
                onChange={v =>
                  setF({ ...f, notification_retention_days: Number(v ?? 30) })
                }
              />
            </Flex>
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
