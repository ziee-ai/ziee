import { Badge, Popover, Typography } from 'antd'
import { DownloadOutlined } from '@ant-design/icons'
import { useLlmModelDownloadStore } from '@/modules/llm-provider/store'
import { DownloadItem } from '../downloads/DownloadItem'
import type { DownloadInstance } from '@/api-client/types'

const { Text } = Typography

export function DownloadIndicatorWidget() {
  const downloads = useLlmModelDownloadStore(state => state.downloads)

  // Filter for active downloads
  const activeDownloads = downloads.filter(
    (download: DownloadInstance) =>
      download.status === 'downloading' || download.status === 'pending'
  )

  // Check if any downloads have failed
  const hasFailedDownloads = downloads.some(
    (download: DownloadInstance) => download.status === 'failed'
  )

  // Hide widget if no active or failed downloads
  if (activeDownloads.length === 0 && !hasFailedDownloads) {
    return null
  }

  const badgeCount = activeDownloads.length
  const badgeColor = hasFailedDownloads ? 'red' : 'blue'

  const popoverContent = (
    <div style={{ width: 300, maxHeight: 400, overflowY: 'auto' }}>
      {activeDownloads.length > 0 ? (
        <>
          <Text strong style={{ display: 'block', marginBottom: 12 }}>
            Active Downloads ({activeDownloads.length})
          </Text>
          {activeDownloads.map(download => (
            <DownloadItem
              key={download.id}
              download={download}
              mode="minimal"
            />
          ))}
        </>
      ) : (
        <Text type="secondary">No active downloads</Text>
      )}
      {hasFailedDownloads && (
        <Text type="danger" style={{ display: 'block', marginTop: 8 }}>
          Some downloads have failed. Check provider settings for details.
        </Text>
      )}
    </div>
  )

  return (
    <Popover
      content={popoverContent}
      title="Downloads"
      trigger="click"
      placement="rightBottom"
    >
      <div
        style={{
          padding: '12px 16px',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <Badge count={badgeCount} color={badgeColor} offset={[10, 0]}>
          <DownloadOutlined style={{ fontSize: 20 }} />
        </Badge>
      </div>
    </Popover>
  )
}
