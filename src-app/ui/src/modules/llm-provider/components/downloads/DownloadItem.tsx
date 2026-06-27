import { Button, Card, Flex, Space, Tag, Tooltip, Text } from '@/components/ui'
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
        return <Tag tone="info">Downloading...</Tag>
      case 'completed':
        return (
          <Tag tone="success" icon={<CheckCircleOutlined />}>
            Downloaded
          </Tag>
        )
      case 'failed':
        return (
          <Tag tone="error" icon={<ExclamationCircleOutlined />}>
            Failed
          </Tag>
        )
      case 'cancelled':
        return <Tag>Cancelled</Tag>
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
      <Card size="sm">
        <Flex vertical gap="small" className="w-full">
          <div
            className="flex justify-between items-center"
          >
            <Space>
              <Text strong>{download.request_data.display_name}</Text>
              {renderStatusTag()}
            </Space>
            <Space>
              {onViewDetails && (
                <Button
                  variant="link"
                  size="sm"
                  icon={<EyeOutlined />}
                  onClick={onViewDetails}
                >
                  View Details
                </Button>
              )}
              {isActive && onCancel && (
                <Button
                  variant="ghost"
                  size="sm"
                  icon={<CloseOutlined />}
                  onClick={onCancel}
                >
                  Cancel
                </Button>
              )}
              {isTerminal && onClose && (
                <Button
                  variant="link"
                  size="sm"
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
        </Flex>
      </Card>
    )
  }

  // COMPACT MODE (for future use)
  if (mode === 'compact') {
    return (
      <div>
        <div
          className="flex justify-between items-center mb-1"
        >
          <span
            className="cursor-pointer text-primary underline underline-offset-2"
            onClick={handleNavigateToProvider}
          >
            {download.request_data.display_name}
          </span>
          {isActive && onCancel && (
            <Button
              variant="ghost"
              size="sm"
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
      <Tooltip content={renderProgressInfo()}>
        <div
          className="mb-2 cursor-pointer"
          onClick={handleNavigateToProvider}
        >
          <div
            className="flex justify-between mb-0.5"
          >
            <Text ellipsis className="text-xs">
              {displayName}
            </Text>
            <Text type="secondary" className="text-xs">
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
