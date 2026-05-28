import { useState } from 'react'
import {
  Alert,
  App,
  Button,
  Card,
  Divider,
  Empty,
  Flex,
  Popconfirm,
  Spin,
  Switch,
  Tag,
  Tooltip,
  Typography,
} from 'antd'
import { LockOutlined } from '@ant-design/icons'
import { Permissions, type AuthProviderResponse } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions/Can'
import { AddProviderMenu } from './AddProviderMenu'
import { AuthProviderEditDrawer } from './AuthProviderEditDrawer'
import type { ProviderTemplate } from '../types'

const { Text } = Typography

type DrawerState =
  | { mode: 'closed' }
  | { mode: 'create'; template: ProviderTemplate }
  | { mode: 'edit'; existing: AuthProviderResponse }

/// Render the relative time for a TIMESTAMPTZ string. Small dependency-
/// free helper — Intl.RelativeTimeFormat would be heavier than what
/// this UI needs ("2m ago" / "1h ago" / "3d ago").
function relativeTime(iso: string | null | undefined): string {
  if (!iso) return ''
  const then = new Date(iso).getTime()
  const secs = Math.floor((Date.now() - then) / 1000)
  if (secs < 60) return `${secs}s ago`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`
  return `${Math.floor(secs / 86400)}d ago`
}

export function AuthProvidersListSection() {
  const { message } = App.useApp()
  const { providers, loading, error, testingIds } = Stores.AuthProvidersAdmin
  const [drawer, setDrawer] = useState<DrawerState>({ mode: 'closed' })
  const [pendingToggleId, setPendingToggleId] = useState<string | null>(null)

  const onToggle = async (row: AuthProviderResponse, next: boolean) => {
    setPendingToggleId(row.id)
    try {
      await Stores.AuthProvidersAdmin.updateProvider(row.id, { enabled: next })
      message.success(next ? 'Provider enabled' : 'Provider disabled')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to update')
    } finally {
      setPendingToggleId(null)
    }
  }

  const onTest = async (row: AuthProviderResponse) => {
    const res = await Stores.AuthProvidersAdmin.testProvider(row.id)
    if (res.ok) {
      message.success(`${row.name}: ${res.message}`)
    } else {
      message.error(`${row.name}: ${res.message}`)
    }
  }

  const onDelete = async (row: AuthProviderResponse) => {
    try {
      await Stores.AuthProvidersAdmin.deleteProvider(row.id)
      message.success(`Deleted ${row.name}`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to delete provider')
    }
  }

  const renderLastTest = (row: AuthProviderResponse) => {
    if (row.last_test_ok === null || row.last_test_ok === undefined) {
      return (
        <Text type="secondary" className="text-xs">
          Last test: never
        </Text>
      )
    }
    const when = relativeTime(row.last_test_at)
    const tip = row.last_test_message ?? ''
    return row.last_test_ok ? (
      <Tooltip title={tip}>
        <Text type="success" className="text-xs">
          ✓ Last test: ok ({when})
        </Text>
      </Tooltip>
    ) : (
      <Tooltip title={tip}>
        <Text type="danger" className="text-xs">
          ✗ Last test: fail ({when})
        </Text>
      </Tooltip>
    )
  }

  const renderRowActions = (row: AuthProviderResponse) => (
    <Can permission={Permissions.AuthProvidersManage}>
      <Flex align="center" gap="small" wrap>
        <Switch
          size="small"
          checked={row.enabled}
          loading={pendingToggleId === row.id}
          onChange={next => onToggle(row, next)}
          aria-label={`Toggle ${row.name}`}
        />
        <Button
          type="text"
          size="small"
          loading={testingIds.has(row.id)}
          onClick={() => onTest(row)}
        >
          Test
        </Button>
        <Button
          type="text"
          size="small"
          onClick={() => setDrawer({ mode: 'edit', existing: row })}
        >
          Edit
        </Button>
        <Popconfirm
          title={`Delete ${row.name}?`}
          description="Linked users lose this sign-in method; their accounts remain."
          okText="Delete"
          okButtonProps={{ danger: true }}
          cancelText="Cancel"
          onConfirm={() => onDelete(row)}
        >
          <Button type="text" size="small" danger>
            Delete
          </Button>
        </Popconfirm>
      </Flex>
    </Can>
  )

  return (
    <>
      <Card
        title="Configured providers"
        extra={
          <Can permission={Permissions.AuthProvidersManage}>
            <AddProviderMenu
              existingNames={providers.map(p => p.name)}
              onPick={template => setDrawer({ mode: 'create', template })}
            />
          </Can>
        }
      >
        {error && (
          <Alert
            type="error"
            title={error}
            showIcon
            closable
            className="mb-3"
          />
        )}

        {loading && providers.length === 0 ? (
          <div className="flex justify-center py-6">
            <Spin />
          </div>
        ) : providers.length === 0 ? (
          <Empty
            description="No providers yet"
            image={<LockOutlined className="text-4xl opacity-50" />}
          >
            <Text type="secondary">
              Use the + button to add Google, Microsoft, Apple, or a custom
              OIDC IdP.
            </Text>
          </Empty>
        ) : (
          <Flex className="flex-col gap-4">
            <div>
              {providers.map((row, index) => (
                <div key={row.id}>
                  <div className="flex items-start gap-3 flex-wrap">
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-2 flex-wrap-reverse">
                        <div className="flex-1 min-w-48">
                          <Flex align="center" gap="small">
                            <Text className="font-medium">{row.name}</Text>
                            <Tag>{row.provider_type}</Tag>
                            {!row.enabled && (
                              <Text type="secondary" className="text-xs">
                                (Disabled)
                              </Text>
                            )}
                          </Flex>
                        </div>
                        <div className="flex gap-1 items-center justify-end">
                          {renderRowActions(row)}
                        </div>
                      </div>

                      <div className="space-y-1">{renderLastTest(row)}</div>
                    </div>
                  </div>
                  {index < providers.length - 1 && (
                    <Divider className="my-4" />
                  )}
                </div>
              ))}
            </div>
          </Flex>
        )}
      </Card>

      <AuthProviderEditDrawer
        open={drawer.mode !== 'closed'}
        template={drawer.mode === 'create' ? drawer.template : undefined}
        existing={drawer.mode === 'edit' ? drawer.existing : undefined}
        onClose={() => setDrawer({ mode: 'closed' })}
      />
    </>
  )
}
