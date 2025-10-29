import { App, Flex, Typography } from 'antd'
import { useEffect } from 'react'
import { useParams } from 'react-router-dom'
import { clearLlmProviderStoreError, Stores } from '../store'
import { ProviderHeader } from './ProviderHeader'

const { Text } = Typography

export function LocalProviderSettings() {
  const { message } = App.useApp()
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
      clearLlmProviderStoreError()
    }
  }, [error, message])

  // Return early if no provider or not local
  if (!currentProvider || currentProvider.provider_type !== 'local') {
    return null
  }

  return (
    <Flex className={'flex-col gap-3 w-full'}>
      <ProviderHeader />

      <Text type="secondary">
        Local providers use models running on your local machine. Configure your local
        inference server separately.
      </Text>
    </Flex>
  )
}
