import { useState } from 'react'
import {
  Alert,
  App,
  Badge,
  Button,
  Card,
  Space,
  Switch,
  Table,
  Tag,
  Tooltip,
  Typography,
} from 'antd'
import type { ColumnsType } from 'antd/es/table'
import { Permissions, type AuthProviderResponse } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions/Can'
import { usePermission } from '@/core/permissions/usePermission'
import { AddProviderMenu } from './AddProviderMenu'
import { AuthProviderEditDrawer } from './AuthProviderEditDrawer'
import { DeleteProviderModal } from './DeleteProviderModal'
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
  const canManage = usePermission(Permissions.AuthProvidersManage)
  const [drawer, setDrawer] = useState<DrawerState>({ mode: 'closed' })
  const [toDelete, setToDelete] = useState<AuthProviderResponse | null>(null)

  const onToggle = async (row: AuthProviderResponse, next: boolean) => {
    try {
      await Stores.AuthProvidersAdmin.updateProvider(row.id, { enabled: next })
      message.success(next ? 'Provider enabled' : 'Provider disabled')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to update')
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

  const columns: ColumnsType<AuthProviderResponse> = [
    {
      title: 'Name',
      dataIndex: 'name',
      key: 'name',
      render: (v: string) => <Text strong>{v}</Text>,
    },
    {
      title: 'Type',
      dataIndex: 'provider_type',
      key: 'provider_type',
      render: (v: string) => <Tag>{v}</Tag>,
    },
    {
      title: 'Status',
      key: 'status',
      render: (_: any, row) => (
        <Badge
          status={row.enabled ? 'success' : 'default'}
          text={row.enabled ? 'Enabled' : 'Disabled'}
        />
      ),
    },
    {
      title: 'Last test',
      key: 'last_test',
      render: (_: any, row) => {
        if (row.last_test_ok === null || row.last_test_ok === undefined) {
          return <Text type="secondary">never</Text>
        }
        const when = relativeTime(row.last_test_at)
        const tip = row.last_test_message ?? ''
        return row.last_test_ok ? (
          <Tooltip title={tip}>
            <Text type="success">✓ ok ({when})</Text>
          </Tooltip>
        ) : (
          <Tooltip title={tip}>
            <Text type="danger">✗ fail ({when})</Text>
          </Tooltip>
        )
      },
    },
    {
      title: 'Actions',
      key: 'actions',
      render: (_: any, row) => (
        <Space wrap>
          {/* Test endpoint requires AuthProvidersManage — gate the
              button consistently so reader users don't see a button
              that 403s on click. */}
          <Can permission={Permissions.AuthProvidersManage}>
            <Button
              size="small"
              loading={testingIds.has(row.id)}
              onClick={() => onTest(row)}
            >
              Test
            </Button>
            <Button
              size="small"
              onClick={() => setDrawer({ mode: 'edit', existing: row })}
            >
              Edit
            </Button>
            <Switch
              size="small"
              checked={row.enabled}
              onChange={next => onToggle(row, next)}
            />
            <Button size="small" danger onClick={() => setToDelete(row)}>
              Delete
            </Button>
          </Can>
        </Space>
      ),
    },
  ]

  return (
    <>
      <Card
        title="Configured providers"
        extra={
          <AddProviderMenu
            disabled={!canManage}
            existingNames={providers.map(p => p.name)}
            onPick={template => setDrawer({ mode: 'create', template })}
          />
        }
      >
        {error && (
          <Alert
            type="error"
            message={error}
            showIcon
            closable
            className="mb-3"
          />
        )}
        <Table
          rowKey="id"
          dataSource={providers}
          columns={columns}
          loading={loading}
          pagination={false}
          locale={{
            emptyText:
              'No providers configured. Click "Add provider" to set up Google, Microsoft, Apple, or a custom OIDC IdP.',
          }}
        />
      </Card>

      <AuthProviderEditDrawer
        open={drawer.mode !== 'closed'}
        template={drawer.mode === 'create' ? drawer.template : undefined}
        existing={drawer.mode === 'edit' ? drawer.existing : undefined}
        onClose={() => setDrawer({ mode: 'closed' })}
      />

      <DeleteProviderModal
        open={toDelete !== null}
        provider={toDelete}
        onClose={() => setToDelete(null)}
      />
    </>
  )
}
