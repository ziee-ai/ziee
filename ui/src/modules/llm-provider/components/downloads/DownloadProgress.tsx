import { Progress } from 'antd'
import type { DownloadStatus } from '@/api-client/types'

interface DownloadProgressProps {
  current: number
  total: number
  status: DownloadStatus
  size?: 'small' | 'default'
}

export function DownloadProgress({
  current,
  total,
  status,
  size = 'default',
}: DownloadProgressProps) {
  const percentage = total > 0 ? Math.round((current / total) * 100) : 0

  const getStatus = () => {
    switch (status) {
      case 'completed':
        return 'success'
      case 'failed':
      case 'cancelled':
        return 'exception'
      case 'downloading':
        return 'active'
      default:
        return 'normal'
    }
  }

  return (
    <Progress
      percent={percentage}
      status={getStatus()}
      size={size}
      showInfo={size !== 'small'}
    />
  )
}
