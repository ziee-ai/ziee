import {
  Alert,
  Button,
  Card,
  Divider,
  Empty,
  Flex,
  Popconfirm,
  Progress,
  Spin,
  Tag,
  Tooltip,
  Typography,
} from 'antd'
import {
  CheckCircleTwoTone,
  CloudDownloadOutlined,
  DeleteOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { formatBytes } from '@/modules/hardware/utils/formatBytes'
import {
  Permissions,
  type EnvironmentInfo,
  type FetchPhase,
} from '@/api-client/types'

const { Text } = Typography

const MANAGE_PERM = Permissions.CodeSandboxEnvironmentsManage
const READ_PERM = Permissions.CodeSandboxEnvironmentsRead

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

/**
 * Rootfs environments admin section. Rendered as a `<Card>` inside the parent
 * `SandboxSettingsPage`. Owns the flavor list, prefetch action (with live
 * SSE progress), and evict action. The card body shows a permission-denied
 * alert when the viewer lacks `code_sandbox::environments::read` (so the
 * section just becomes a "you don't have access" stub rather than vanishing
 * — matches how UsersSettings handles read-vs-manage).
 */
export function SandboxEnvironmentsSection() {
  const { environments, loading, error, progress, evicting } =
    Stores.SandboxEnvironments
  const canManage = usePermission(MANAGE_PERM)
  const canRead = usePermission(READ_PERM) || canManage

  if (!canRead) {
    return (
      <Card title="Rootfs environments">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view sandbox environments."
        />
      </Card>
    )
  }

  // Renders the right-side action button (Fetch / Evict) plus
  // the inline progress bar when a fetch is in flight. Same logic
  // as the previous Table render, just lifted out for clarity.
  const renderRowActions = (row: EnvironmentInfo) => {
    const p = progress[row.flavor]
    const busy = p?.status === 'running'

    if (row.cached && p?.status !== 'failed') {
      const evictBtn = (
        <Popconfirm
          title="Evict cached rootfs?"
          description={
            row.mounted
              ? 'This flavor is mounted; evicting unmounts it. An in-flight execution may fail and the next one re-downloads.'
              : 'Frees disk; the next code execution for this flavor re-downloads it.'
          }
          okText="Evict"
          okButtonProps={{ danger: true }}
          onConfirm={() =>
            Stores.SandboxEnvironments.evictEnvironment(row.flavor)
          }
        >
          <Button
            danger
            type="text"
            icon={<DeleteOutlined />}
            loading={!!evicting[row.flavor]}
            disabled={!canManage}
            data-testid="evict-button"
          >
            Evict
          </Button>
        </Popconfirm>
      )
      return canManage ? (
        evictBtn
      ) : (
        <Tooltip title={`Requires ${MANAGE_PERM}`}>{evictBtn}</Tooltip>
      )
    }

    const btn = (
      <Button
        type="text"
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
      <Tooltip title={`Requires ${MANAGE_PERM}`}>{btn}</Tooltip>
    )
  }

  const renderStatus = (row: EnvironmentInfo) => {
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
  }

  return (
    <Card title="Rootfs environments">
      {error && <Alert type="error" showIcon title={error} className="mb-3" />}
      {loading ? (
        <Spin />
      ) : (
        <Flex className="flex-col gap-4">
          {environments.length === 0 ? (
            <Empty
              description="No environments available"
              image={<CloudDownloadOutlined className="text-4xl opacity-50" />}
            />
          ) : (
            <div>
              {environments.map((row, index) => (
                <div key={row.flavor} data-flavor={row.flavor}>
                  <div className="flex items-start gap-3 flex-wrap">
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-2 flex-wrap-reverse">
                        <div className="flex-1 min-w-48">
                          <Flex align="center" gap="small">
                            <Text className="font-medium">{row.flavor}</Text>
                            {row.mounted && (
                              <Tag color="blue" className="!m-0">
                                Mounted
                              </Tag>
                            )}
                          </Flex>
                        </div>
                        <div className="flex gap-1 items-center justify-end">
                          {renderRowActions(row)}
                        </div>
                      </div>

                      <div className="space-y-1">
                        <Text type="secondary" className="block">
                          {row.description}
                        </Text>
                        <Text type="secondary" className="text-xs block">
                          Size: ~{row.approximate_size_mb} MB
                          {row.cached && row.cached_size_bytes != null && (
                            <>
                              {' · '}
                              <span data-testid="cached-size">
                                Cached: {formatBytes(row.cached_size_bytes)}
                              </span>
                            </>
                          )}
                        </Text>
                        <div className="pt-1">{renderStatus(row)}</div>
                      </div>
                    </div>
                  </div>
                  {index < environments.length - 1 && (
                    <Divider className="my-4" />
                  )}
                </div>
              ))}
            </div>
          )}
        </Flex>
      )}
    </Card>
  )
}
