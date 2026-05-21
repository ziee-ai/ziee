import { Alert, Button, Progress, Spin, Table, Tag, Tooltip } from 'antd'
import { CheckCircleTwoTone, CloudDownloadOutlined } from '@ant-design/icons'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Stores } from '@/core/stores'
import type { EnvironmentInfo, FetchPhase } from '@/api-client/types'

const MANAGE_PERM = 'code_sandbox::environments::manage'
const READ_PERM = 'code_sandbox::environments::read'

// Mirror the backend's permission matching (auth/backend.rs::has_permission):
// honor the global `*` wildcard (the Administrators group is seeded with
// ARRAY['*']) and a `resource:*` wildcard, in addition to an exact match.
function hasPermission(perms: string[], perm: string): boolean {
  if (perms.includes('*')) return true
  if (perms.includes(perm)) return true
  const idx = perm.indexOf(':')
  if (idx > 0 && perms.includes(`${perm.slice(0, idx)}:*`)) return true
  return false
}

// The backend emits discrete phase events (resolving/downloading/
// verifying/installing), not byte-granular progress, so the bar is a
// coarse stepped indicator keyed off the current phase.
function phasePercent(phase?: FetchPhase): number {
  switch (phase) {
    case 'resolving':
      return 10
    case 'downloading':
      return 50
    case 'verifying_sha256':
      return 75
    case 'verifying_cosign':
      return 85
    case 'installing':
      return 95
    default:
      return 5
  }
}

export function SandboxEnvironmentsPage() {
  const { environments, loading, error, progress } = Stores.SandboxEnvironments
  const { permissions } = Stores.Auth
  const perms = permissions ?? []
  const canManage = hasPermission(perms, MANAGE_PERM)
  const canRead = hasPermission(perms, READ_PERM) || canManage

  if (!canRead) {
    return (
      <SettingsPageContainer title="Sandbox Environments">
        <Alert
          type="warning"
          showIcon
          message="You don't have permission to view sandbox environments."
        />
      </SettingsPageContainer>
    )
  }

  const columns = [
    {
      title: 'Environment',
      dataIndex: 'flavor',
      key: 'flavor',
      render: (flavor: string, row: EnvironmentInfo) => (
        <div>
          <strong>{flavor}</strong>
          <div className="text-xs opacity-70">{row.description}</div>
        </div>
      ),
    },
    {
      title: 'Size',
      dataIndex: 'approximate_size_mb',
      key: 'size',
      render: (mb: number) => `~${mb} MB`,
    },
    {
      title: 'Status',
      key: 'status',
      render: (_: unknown, row: EnvironmentInfo) => {
        const p = progress[row.flavor]
        if (row.cached && (!p || p.status === 'completed')) {
          return (
            <Tag
              icon={<CheckCircleTwoTone twoToneColor="#52c41a" />}
              color="success"
            >
              Cached
            </Tag>
          )
        }
        if (p?.status === 'running') {
          return (
            <div style={{ minWidth: 180 }} data-testid="prefetch-progress">
              <Progress
                percent={phasePercent(p.phase)}
                size="small"
                status="active"
              />
              <div className="text-xs opacity-70">{p.message ?? p.phase}</div>
            </div>
          )
        }
        if (p?.status === 'failed') {
          return <Tag color="error">Failed: {p.error}</Tag>
        }
        return <Tag>Not fetched</Tag>
      },
    },
    {
      title: '',
      key: 'action',
      render: (_: unknown, row: EnvironmentInfo) => {
        const p = progress[row.flavor]
        const busy = p?.status === 'running'
        // Hide the button when cached + not in a failed state.
        if (row.cached && p?.status !== 'failed') return null
        const btn = (
          <Button
            type="primary"
            icon={<CloudDownloadOutlined />}
            loading={busy}
            disabled={!canManage || busy}
            onClick={() => Stores.SandboxEnvironments.startPrefetch(row.flavor)}
          >
            Fetch
          </Button>
        )
        return canManage ? (
          btn
        ) : (
          <Tooltip title="Requires code_sandbox::environments::manage">
            {btn}
          </Tooltip>
        )
      },
    },
  ]

  return (
    <SettingsPageContainer
      title="Sandbox Environments"
      subtitle="Pre-fetch rootfs flavors so the first code execution doesn't pay download latency."
    >
      {error && <Alert type="error" showIcon message={error} />}
      {loading ? (
        <Spin />
      ) : (
        <Table
          rowKey="flavor"
          dataSource={environments}
          columns={columns}
          pagination={false}
          onRow={row => ({ 'data-flavor': row.flavor }) as any}
        />
      )}
    </SettingsPageContainer>
  )
}
