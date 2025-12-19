import { Button, Card, Space, Tag, Tooltip, Typography } from 'antd'
import {
  CloseOutlined,
  CheckCircleOutlined,
  ExclamationCircleOutlined,
  EyeOutlined,
} from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { DownloadProgress } from '@/modules/llm-provider/components/downloads/DownloadProgress'
import { formatBytes, formatSpeed, formatETA } from '@/utils/downloadUtils'
import type { DownloadInstance } from '@/api-client/types'

const { Text, Link } = Typography

interface DownloadItemProps {
  download: DownloadInstance
  mode: 'full' | 'compact' | 'minimal'
  onCancel?: () => void
  onClose?: () => void
  onViewDetails?: () => void
}

export function DownloadItem({
  download,
  mode,
  onCancel,
  onClose,
  onViewDetails,
}: DownloadItemProps) {
  const navigate = useNavigate()

  const isActive =
    download.status === 'downloading' || download.status === 'pending'
  const isTerminal =
    download.status === 'completed' ||
    download.status === 'failed' ||
    download.status === 'cancelled'

  const handleNavigateToProvider = () => {
    navigate(`/settings/llm-providers/${download.provider_id}`)
  }

  const renderStatusTag = () => {
    switch (download.status) {
      case 'downloading':
      case 'pending':
        return <Tag color="blue">Downloading...</Tag>
      case 'completed':
        return (
          <Tag color="green" icon={<CheckCircleOutlined />}>
            Downloaded
          </Tag>
        )
      case 'failed':
        return (
          <Tag color="red" icon={<ExclamationCircleOutlined />}>
            Failed
          </Tag>
        )
      case 'cancelled':
        return <Tag color="default">Cancelled</Tag>
      default:
        return null
    }
  }

  const renderProgressInfo = () => {
    const { progress_data } = download
    if (!progress_data) return null

    const { current, total, speed_bps, eta_seconds } = progress_data

    return (
      <Space size="small">
        <Text type="secondary">
          {formatBytes(current)} / {formatBytes(total)}
        </Text>
        {speed_bps > 0 && (
          <>
            <Text type="secondary">•</Text>
            <Text type="secondary">{formatSpeed(speed_bps)}</Text>
          </>
        )}
        {eta_seconds > 0 && (
          <>
            <Text type="secondary">•</Text>
            <Text type="secondary">ETA: {formatETA(eta_seconds)}</Text>
          </>
        )}
      </Space>
    )
  }

  // FULL MODE (for LocalProviderSettings)
  if (mode === 'full') {
    return (
      <Card size="small">
        <Space direction="vertical" style={{ width: '100%' }} size="small">
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <Space>
              <Text strong>{download.request_data.display_name}</Text>
              {renderStatusTag()}
            </Space>
            <Space>
              {onViewDetails && (
                <Button
                  type="link"
                  size="small"
                  icon={<EyeOutlined />}
                  onClick={onViewDetails}
                >
                  View Details
                </Button>
              )}
              {isActive && onCancel && (
                <Button
                  type="link"
                  size="small"
                  danger
                  icon={<CloseOutlined />}
                  onClick={onCancel}
                >
                  Cancel
                </Button>
              )}
              {isTerminal && onClose && (
                <Button
                  type="link"
                  size="small"
                  icon={<CloseOutlined />}
                  onClick={onClose}
                >
                  Close
                </Button>
              )}
            </Space>
          </div>

          {download.request_data.description && (
            <Text type="secondary">{download.request_data.description}</Text>
          )}

          <DownloadProgress
            current={download.progress_data?.current || 0}
            total={download.progress_data?.total || 0}
            status={download.status}
          />

          {renderProgressInfo()}

          {download.error_message && (
            <Text type="danger">{download.error_message}</Text>
          )}
        </Space>
      </Card>
    )
  }

  // COMPACT MODE (for future use)
  if (mode === 'compact') {
    return (
      <div>
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: 4,
          }}
        >
          <Link onClick={handleNavigateToProvider}>
            {download.request_data.display_name}
          </Link>
          {isActive && onCancel && (
            <Button
              type="link"
              size="small"
              danger
              icon={<CloseOutlined />}
              onClick={onCancel}
            >
              Cancel
            </Button>
          )}
        </div>
        <DownloadProgress
          current={download.progress_data?.current || 0}
          total={download.progress_data?.total || 0}
          status={download.status}
          size="small"
        />
        {renderProgressInfo()}
      </div>
    )
  }

  // MINIMAL MODE (for DownloadIndicator widget)
  if (mode === 'minimal') {
    const fullName = download.request_data.display_name || 'Unnamed Model'
    const displayName =
      fullName.length > 30 ? fullName.substring(0, 30) + '...' : fullName

    return (
      <Tooltip title={renderProgressInfo()}>
        <div
          style={{ marginBottom: 8, cursor: 'pointer' }}
          onClick={handleNavigateToProvider}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              marginBottom: 2,
            }}
          >
            <Text ellipsis style={{ fontSize: 12 }}>
              {displayName}
            </Text>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {Math.round(
                ((download.progress_data?.current || 0) /
                  (download.progress_data?.total || 1)) *
                  100,
              )}
              %
            </Text>
          </div>
          <DownloadProgress
            current={download.progress_data?.current || 0}
            total={download.progress_data?.total || 0}
            status={download.status}
            size="small"
          />
        </div>
      </Tooltip>
    )
  }

  return null
}
