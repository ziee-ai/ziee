import { Card } from 'antd'
import { DownloadItem } from './DownloadItem'
import {
  cancelLlmModelDownload,
  deleteLlmModelDownload,
  openViewDownloadDrawer,
} from '@/modules/llm-provider/store'
import { Stores } from '@/core/stores'
import type { DownloadInstance } from '@/api-client/types'

interface DownloadsSectionProps {
  providerId: string
}

export function DownloadsSection({ providerId }: DownloadsSectionProps) {
  const { downloads } = Stores.LlmModelDownload

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
    <Card title="Downloading Models" classNames={{body: "flex flex-col gap-3"}}>
      {providerDownloads.map((download) => (
        <div key={download.id}>
          <DownloadItem
            download={download}
            mode="full"
            onCancel={() => handleCancelDownload(download.id)}
            onClose={() => handleCloseDownload(download.id)}
            onViewDetails={() => handleViewDetails(download.id)}
          />
        </div>
      ))}
    </Card>
  )
}
