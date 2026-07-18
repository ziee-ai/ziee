import { Progress } from '@ziee/kit'
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

  const tone = () => {
    switch (status) {
      case 'completed':
        return 'success'
      case 'failed':
      case 'cancelled':
        return 'error'
      case 'downloading':
        return 'primary'
      default:
        return 'primary'
    }
  }

  return (
    <Progress
      value={percentage}
      tone={tone()}
      size={size === 'small' ? 'sm' : undefined}
      showInfo={size !== 'small'}
      aria-label={`Download progress: ${percentage}%`}
      data-testid="llm-download-progress"
    />
  )
}
