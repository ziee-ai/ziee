import { Card, Divider, Typography } from 'antd'
import { DownloadItem } from './DownloadItem'
import {
  cancelLlmModelDownload,
  deleteLlmModelDownload,
  useLlmModelDownloadStore,
  openViewDownloadDrawer,
} from '@/modules/llm-provider/store'
import type { DownloadInstance } from '@/api-client/types'

const { Title } = Typography

interface DownloadsSectionProps {
  providerId: string
}

export function DownloadsSection({ providerId }: DownloadsSectionProps) {
  const downloads = useLlmModelDownloadStore(state => state.downloads)

  // Filter downloads for this provider
  const providerDownloads = downloads.filter(
    (download: DownloadInstance) => download.provider_id === providerId
  )

  if (providerDownloads.length === 0) {
    return null
  }

  const handleCancelDownload = async (downloadId: string) => {
    try {
      await cancelLlmModelDownload(downloadId)
    } catch (error) {
      console.error('Failed to cancel download:', error)
    }
  }

  const handleCloseDownload = async (downloadId: string) => {
    try {
      await deleteLlmModelDownload(downloadId)
    } catch (error) {
      console.error('Failed to delete download:', error)
    }
  }

  const handleViewDetails = (downloadId: string) => {
    openViewDownloadDrawer(downloadId)
  }

  return (
    <Card style={{ marginBottom: 24 }}>
      <Title level={4}>Downloading Models</Title>
      {providerDownloads.map((download, index) => (
        <div key={download.id}>
          <DownloadItem
            download={download}
            mode="full"
            onCancel={() => handleCancelDownload(download.id)}
            onClose={() => handleCloseDownload(download.id)}
            onViewDetails={() => handleViewDetails(download.id)}
          />
          {index < providerDownloads.length - 1 && <Divider />}
        </div>
      ))}
    </Card>
  )
}
