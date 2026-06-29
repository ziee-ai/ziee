import { Flex, Text, message } from '@/components/ui'
import { useEffect } from 'react'
import { useParams } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { ProviderHeader } from '@/modules/llm-provider/components/ProviderHeader'
import { LlmModelsSection } from '@/modules/llm-provider/components/LlmModelsSection'
import { ProviderGroupAssignmentCard } from '@/modules/llm-provider/components/ProviderGroupAssignmentCard'
import { DownloadsSection } from '@/modules/llm-provider/components/downloads/DownloadsSection'
import { AddLocalLlmModelUploadDrawer } from '@/modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer'
import { AddLocalLlmModelDownloadDrawer } from '@/modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer'
import { EditLlmModelDrawer } from '@/modules/llm-provider/components/llm-models/EditLlmModelDrawer'

export function LocalProviderSettings() {
  console.log('[LocalProviderSettings] Component rendering')
  const { providerId } = useParams<{ providerId?: string }>()

  // Store data
  const { error } = Stores.LlmProvider

  // Get current provider
  const currentProvider = Stores.LlmProvider.providers.find(
    p => p.id === providerId,
  )

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.LlmProvider.clearLlmProviderStoreError()
    }
  }, [error])

  // Return early if no provider or not local
  if (!currentProvider || currentProvider.provider_type !== 'local') {
    return null
  }

  return (
    <Flex className={'flex-col gap-3 w-full'}>
      <ProviderHeader />

      <Text type="secondary">
        Local providers use models running on your local machine. Configure your
        local inference server separately.
      </Text>

      <DownloadsSection providerId={currentProvider.id} />

      <LlmModelsSection />

      {/* User Groups Assignment - After Models */}
      <ProviderGroupAssignmentCard />

      {/* Model Management Drawers */}
      <AddLocalLlmModelUploadDrawer />
      <AddLocalLlmModelDownloadDrawer />
      <EditLlmModelDrawer />
    </Flex>
  )
}
