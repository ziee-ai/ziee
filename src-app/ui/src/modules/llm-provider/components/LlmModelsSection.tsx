import {
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
  UploadOutlined,
} from '@ant-design/icons'
import {
  Button,
  Card,
  Separator,
  Dropdown,
  Empty,
  Flex,
  Switch,
  Tooltip,
  Text,
} from '@/components/ui'
import { message } from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { useParams } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type LlmModel } from '@/api-client/types'

export function LlmModelsSection() {
  const { providerId } = useParams<{ providerId?: string }>()

  // Store data
  const { llmModelsLoading } = Stores.LlmProvider
  const canEditModels = usePermission(Permissions.LlmModelsEdit)
  const canDeleteModels = usePermission(Permissions.LlmModelsDelete)
  const canCreateModels = usePermission(Permissions.LlmModelsCreate)

  // Get current provider and its models
  const currentProvider = Stores.LlmProvider.providers.find(
    p => p.id === providerId,
  )
  const llmModels = currentProvider?.llm_models || []
  const loading = llmModelsLoading?.[providerId!] || false

  const handleToggleLlmModel = async (modelId: string, enabled: boolean) => {
    if (!currentProvider) return

    try {
      if (enabled) {
        await Stores.LlmProvider.enableLlmModel(modelId)
      } else {
        await Stores.LlmProvider.disableLlmModel(modelId)
      }

      // Check if this was the last enabled model being disabled
      if (!enabled) {
        const remainingEnabledModels = llmModels.filter(
          m => m.id !== modelId && m.enabled !== false,
        )

        // If no models remain enabled and provider is currently enabled, disable the provider
        if (remainingEnabledModels.length === 0 && currentProvider.enabled) {
          try {
            await Stores.LlmProvider.updateLlmProvider(currentProvider.id, {
              enabled: false,
            })
            const modelName =
              llmModels.find(m => m.id === modelId)?.name || 'Model'
            message.success(
              `${modelName} disabled. ${currentProvider.name} provider disabled as no models remain active.`,
            )
          } catch (providerError) {
            console.error('Failed to disable provider:', providerError)
            const modelName =
              llmModels.find(m => m.id === modelId)?.name || 'Model'
            message.warning(
              `${modelName} disabled, but failed to disable provider automatically`,
            )
          }
        } else {
          const modelName =
            llmModels.find(m => m.id === modelId)?.name || 'Model'
          message.success(`${modelName} ${enabled ? 'enabled' : 'disabled'}`)
        }
      } else {
        const modelName = llmModels.find(m => m.id === modelId)?.name || 'Model'
        message.success(`${modelName} ${enabled ? 'enabled' : 'disabled'}`)
      }
    } catch (error) {
      console.error('Failed to toggle model:', error)
      // Error is handled by the store
    }
  }

  const handleDeleteLlmModel = async (modelId: string) => {
    if (!currentProvider) return

    try {
      await Stores.LlmProvider.deleteLlmModel(modelId)
      message.success('Model deleted')
    } catch (error) {
      console.error('Failed to delete model:', error)
      // Error is handled by the store
    }
  }

  // TODO: Implement start/stop functionality for local models once backend supports it
  // const handleStartStopLlmModel = async (modelId: string, is_active: boolean) => {
  //   if (!currentProvider || currentProvider.provider_type !== 'local') return
  //   ...
  // }

  const handleAddLlmModel = () => {
    if (!currentProvider) return
    if (currentProvider.provider_type === 'local') {
      // For local providers, open the upload drawer by default
      Stores.AddLocalLlmModelUploadDrawer.openAddLocalLlmModelUploadDrawer(
        currentProvider.id,
      )
    } else {
      Stores.AddRemoteLlmModelDrawer.openAddRemoteLlmModelDrawer(
        currentProvider.id,
        currentProvider.provider_type,
      )
    }
  }

  const handleEditLlmModel = (modelId: string) => {
    if (!currentProvider) return
    Stores.EditLlmModelDrawer.openEditLlmModelDrawer(modelId)
  }

  const getLlmModelActions = (llmModel: LlmModel) => {
    const actions: React.ReactNode[] = []

    // Enable/disable switch — needs edit permission
    if (canEditModels) {
      actions.push(
        <Switch
          className={'!mr-2'}
          key="enable"
          checked={llmModel.enabled !== false}
          onChange={checked => handleToggleLlmModel(llmModel.id, checked)}
          aria-label={`${llmModel.enabled !== false ? 'Disable' : 'Enable'} ${llmModel.display_name} model`}
        />,
      )
    }

    // TODO: Add Start/Stop button for local models once backend supports it
    // if (currentProvider?.provider_type === 'local') {
    //   actions.push(
    //     <Button
    //       key="start-stop"
    //       type={llmModel.is_active ? 'default' : 'primary'}
    //       loading={llmModelOperations?.[llmModel.id] || false}
    //       disabled={llmModelOperations?.[llmModel.id] || false}
    //       onClick={() => handleStartStopLlmModel(llmModel.id, !llmModel.is_active)}
    //     >
    //       {llmModelOperations?.[llmModel.id]
    //         ? llmModel.is_active
    //           ? 'Stopping...'
    //           : 'Starting...'
    //         : llmModel.is_active
    //           ? 'Stop'
    //           : 'Start'}
    //     </Button>,
    //   )
    // }

    if (canEditModels) {
      actions.push(
        <Button
          key="edit"
          variant="ghost"
          icon={<EditOutlined aria-hidden="true" />}
          onClick={() => handleEditLlmModel(llmModel.id)}
          aria-label={`Edit ${llmModel.display_name} model`}
        >
          {'Edit'}
        </Button>,
      )
    }

    if (canDeleteModels) {
      actions.push(
        <Button
          key="delete"
          variant="ghost"
          icon={<DeleteOutlined aria-hidden="true" />}
          onClick={() => handleDeleteLlmModel(llmModel.id)}
          aria-label={`Delete ${llmModel.display_name} model`}
        >
          {'Delete'}
        </Button>,
      )
    }

    return actions.filter(Boolean)
  }

  const getAddButton = () => {
    if (!currentProvider) return null
    if (!canCreateModels) return null

    if (currentProvider.provider_type === 'local') {
      return (
        <Dropdown
          items={[
            {
              key: 'upload',
              label: 'Upload from Files',
              icon: <UploadOutlined />,
              onClick: () =>
                Stores.AddLocalLlmModelUploadDrawer.openAddLocalLlmModelUploadDrawer(
                  currentProvider.id,
                ),
            },
            {
              key: 'download',
              label: 'Download from Repository',
              icon: <PlusOutlined />,
              onClick: () =>
                Stores.AddLocalLlmModelDownloadDrawer.openAddLocalLlmModelDownloadDrawer(
                  currentProvider.id,
                ),
            },
          ]}
        >
          <Tooltip content="Add model">
            <Button
              variant="ghost"
              icon={<PlusOutlined aria-hidden="true" />}
              aria-label="Add model"
            />
          </Tooltip>
        </Dropdown>
      )
    }

    return (
      <Tooltip content="Add model">
        <Button
          variant="ghost"
          icon={<PlusOutlined aria-hidden="true" />}
          onClick={handleAddLlmModel}
          aria-label="Add model"
        />
      </Tooltip>
    )
  }

  // Return early if no provider
  if (!currentProvider) {
    return null
  }

  return (
    <Card title="Models" extra={getAddButton()}>
      {loading ? (
        <Loading />
      ) : llmModels.length === 0 ? (
        <div>
          <Empty description="No models yet" />
        </div>
      ) : (
        <div>
          {llmModels.map((llmModel, index: number) => (
            <div key={llmModel.id}>
              <div className="flex items-start gap-3 flex-wrap">
                {/* Model Info */}
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-2 flex-wrap-reverse">
                    <div className={'flex-1 min-w-48'}>
                      <Text className="font-medium">
                        {llmModel.display_name}
                      </Text>
                      {llmModel.is_deprecated && (
                        <span className="text-xs">⚠️</span>
                      )}
                    </div>
                    <div className={'flex gap-1 items-center justify-end'}>
                      {getLlmModelActions(llmModel)}
                    </div>
                  </div>

                  <div className="space-y-1">
                    <Text type="secondary" className="text-xs block">
                      Model ID: {llmModel.name}
                    </Text>
                    {/* TODO: Display running status once backend supports local model execution */}
                    {/* {llmModel.is_active && llmModel.port && (
                      <Text type="secondary" className="text-xs block">
                        Running on:{' '}
                        <a
                          href={`http://127.0.0.1:${llmModel.port}`}
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          http://127.0.0.1:{llmModel.port}
                        </a>
                      </Text>
                    )} */}
                    {llmModel.description && (
                      <Text type="secondary" className="block">
                        {llmModel.description}
                      </Text>
                    )}
                    {llmModel.capabilities && (
                      <Flex wrap className="gap-3 pt-1 flex-wrap">
                        {llmModel.capabilities.vision && (
                          <Text type="secondary" className="text-xs">
                            👁️ Vision
                          </Text>
                        )}
                        {llmModel.capabilities.audio && (
                          <Text type="secondary" className="text-xs">
                            🎵 Audio
                          </Text>
                        )}
                        {llmModel.capabilities.tools && (
                          <Text type="secondary" className="text-xs">
                            🔧 Tools
                          </Text>
                        )}
                        {llmModel.capabilities.code_interpreter && (
                          <Text type="secondary" className="text-xs">
                            💻 Code
                          </Text>
                        )}
                        {llmModel.capabilities.chat && (
                          <Text type="secondary" className="text-xs">
                            💬 Chat
                          </Text>
                        )}
                        {llmModel.capabilities.text_embedding && (
                          <Text type="secondary" className="text-xs">
                            🔍 Embedding
                          </Text>
                        )}
                        {llmModel.capabilities.image_generator && (
                          <Text type="secondary" className="text-xs">
                            🎨 Image Gen
                          </Text>
                        )}
                      </Flex>
                    )}
                  </div>
                </div>
              </div>
              {index < llmModels.length - 1 && <Separator className="my-0" />}
            </div>
          ))}
        </div>
      )}
    </Card>
  )
}
