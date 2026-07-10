import { Download } from 'lucide-react'
import {
  Button,
  Card,
  ErrorState,
  Flex,
  Spin,
  Tag,
  Text,
  message,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/**
 * Whisper model readiness + download. The model-download endpoint is
 * synchronous (no SSE), so the button shows an indeterminate loading state
 * while the download runs rather than a byte-progress bar.
 */
export function ModelCard() {
  const { status, loading, downloading, error } = Stores.VoiceModel

  const handleDownload = async () => {
    try {
      await Stores.VoiceModel.downloadModel()
      message.success('Model downloaded')
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to download model')
    }
  }

  return (
    <Card title="Model" data-testid="voice-model-card">
      {loading && !status ? (
        <Spin label="Loading" />
      ) : error && !status ? (
        <ErrorState
          resource="model status"
          description="The model status couldn't be loaded."
          details={error}
          onRetry={() => Stores.VoiceModel.loadStatus()}
          data-testid="voice-model-error"
        />
      ) : (
        <Flex justify="between" align="center" gap="small" wrap data-testid="voice-model-row">
          <Flex vertical gap="small">
            <Flex align="center" gap="small" wrap>
              <Text strong>{status?.model ?? 'No model configured'}</Text>
              {status?.present ? (
                <Tag tone="success" variant="outline" data-testid="voice-model-present-tag">
                  present
                </Tag>
              ) : (
                <Tag tone="warning" variant="outline" data-testid="voice-model-missing-tag">
                  not downloaded
                </Tag>
              )}
            </Flex>
            {status?.present && status.size_bytes != null && (
              <Text type="secondary" className="text-xs">
                {formatBytes(status.size_bytes)} on disk
              </Text>
            )}
          </Flex>
          <Can permission={Permissions.VoiceAdminManage}>
            <Button
              icon={<Download />}
              loading={downloading}
              disabled={downloading}
              onClick={handleDownload}
              data-testid="voice-model-download-btn"
              aria-label="Download model"
            >
              {status?.present ? 'Re-download' : downloading ? 'Downloading…' : 'Download'}
            </Button>
          </Can>
        </Flex>
      )}
    </Card>
  )
}
