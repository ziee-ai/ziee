import { Trash2, Pencil, Plus, Upload, RefreshCw } from 'lucide-react'
import {
  Badge,
  Button,
  Card,
  Separator,
  Dropdown,
  Empty,
  Flex,
  Switch,
  Text,
  Tooltip,
} from '@ziee/kit'
import { message } from '@ziee/kit'
import { Loading } from '@/core/components/Loading'
import { useParams } from 'react-router-dom'
import { useState } from 'react'
import { Stores } from '@ziee/framework/stores'
import { ApiClient } from '@/api-client'
import { usePermission } from '@/core/permissions'
import { Permissions, type LlmModel } from '@/api-client/types'

export function LlmModelsSection() {
  const { providerId } = useParams<{ providerId?: string }>()
  // Per-model in-flight flag for the local-runtime start/stop button.
  const [llmModelOperations, setLlmModelOperations] = useState<
    Record<string, boolean>
  >({})

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
      // Surface an actionable message: the store reverts the Switch optimistic
      // state, but without a toast the failure is otherwise invisible. Prefer
      // the backend's readable reason; fall back to a clear generic message.
      const modelName = llmModels.find(m => m.id === modelId)?.name || 'Model'
      const reason =
        error instanceof Error && error.message
          ? error.message
          : `Failed to ${enabled ? 'enable' : 'disable'} ${modelName}.`
      message.error(reason, { duration: 8000 })
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

  // Start/stop a local model's runtime instance via /api/local-runtime, then
  // refresh the provider's models so is_active/port reflect the new state.
  const handleStartStopLlmModel = async (modelId: string, start: boolean) => {
    if (!currentProvider || currentProvider.provider_type !== 'local') return
    setLlmModelOperations(prev => ({ ...prev, [modelId]: true }))
    try {
      if (start) {
        await ApiClient.LocalRuntime.startModel({ model_id: modelId }, undefined)
        message.success('Model starting')
      } else {
        await ApiClient.LocalRuntime.stopModel({ model_id: modelId }, undefined)
        message.success('Model stopped')
      }
      await Stores.LlmProvider.loadModelsForProvider(currentProvider.id)
    } catch (error) {
      console.error('Failed to start/stop model:', error)
      message.error(
        error instanceof Error
          ? error.message
          : `Failed to ${start ? 'start' : 'stop'} model`,
      )
    } finally {
      setLlmModelOperations(prev => ({ ...prev, [modelId]: false }))
    }
  }

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

  // Reconcile this remote provider's saved models against its live model list —
  // flags deprecated/removed ones and clears the flag on any that reappeared.
  const handleRefreshModels = async () => {
    if (!currentProvider) return
    try {
      const models = await Stores.LlmProvider.refreshProviderModels(currentProvider.id)
      const deprecated = models.filter(m => m.is_deprecated).length
      message.success(
        deprecated > 0
          ? `Models refreshed — ${deprecated} unavailable/deprecated`
          : 'Models refreshed — all available',
      )
    } catch (error) {
      console.error('Failed to refresh models:', error)
      message.error(
        error instanceof Error ? error.message : 'Failed to refresh models',
      )
    }
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
          tooltip={`${llmModel.enabled !== false ? 'Disable' : 'Enable'} ${llmModel.display_name} model`}
          data-testid={`llm-model-enable-switch-${llmModel.id}`}
        />,
      )
    }

    // Start/Stop the local-runtime instance (local providers only, edit perm).
    if (canEditModels && currentProvider?.provider_type === 'local') {
      const busy = llmModelOperations[llmModel.id] || false
      actions.push(
        <Button
          key="start-stop"
          data-testid={`llm-model-start-stop-${llmModel.id}`}
          size="default"
          variant={llmModel.is_active ? 'outline' : 'default'}
          loading={busy}
          disabled={busy}
          onClick={() =>
            handleStartStopLlmModel(llmModel.id, !llmModel.is_active)
          }
          aria-label={`${llmModel.is_active ? 'Stop' : 'Start'} ${llmModel.display_name} model`}
        >
          {llmModel.is_active ? 'Stop' : 'Start'}
        </Button>,
      )
    }

    if (canEditModels) {
      actions.push(
        <Button
          key="edit"
          variant="ghost"
          icon={<Pencil aria-hidden="true" />}
          onClick={() => handleEditLlmModel(llmModel.id)}
          aria-label={`Edit ${llmModel.display_name} model`}
          data-testid={`llm-model-edit-btn-${llmModel.id}`}
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
          icon={<Trash2 aria-hidden="true" />}
          onClick={() => handleDeleteLlmModel(llmModel.id)}
          aria-label={`Delete ${llmModel.display_name} model`}
          data-testid={`llm-model-delete-btn-${llmModel.id}`}
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
        // Tooltip OUTERMOST (wrapping the span that wraps the Dropdown) so it
        // doesn't share a trigger element with the Dropdown — the same stable
        // pattern as AddProviderMenu; a Tooltip nested INSIDE the Dropdown
        // double-triggers on the button and flickers.
        <Tooltip content="Add model">
          <span className="inline-flex">
            <Dropdown
              data-testid="llm-models-add-dropdown"
              items={[
                {
                  key: 'upload',
                  label: 'Upload from Files',
                  icon: <Upload />,
                  onClick: () =>
                    Stores.AddLocalLlmModelUploadDrawer.openAddLocalLlmModelUploadDrawer(
                      currentProvider.id,
                    ),
                },
                {
                  key: 'download',
                  label: 'Download from Repository',
                  icon: <Plus />,
                  onClick: () =>
                    Stores.AddLocalLlmModelDownloadDrawer.openAddLocalLlmModelDownloadDrawer(
                      currentProvider.id,
                    ),
                },
              ]}
            >
              <Button
                variant="default"
                size="icon"
                icon={<Plus aria-hidden="true" />}
                aria-label="Add model"
                data-testid="llm-models-add-local-btn"
              />
            </Dropdown>
          </span>
        </Tooltip>
      )
    }

    return (
      <Tooltip content="Add model">
        <span className="inline-flex">
          <Button
            variant="default"
            size="icon"
            icon={<Plus aria-hidden="true" />}
            onClick={handleAddLlmModel}
            aria-label="Add model"
            data-testid="llm-models-add-remote-btn"
          />
        </span>
      </Tooltip>
    )
  }

  // "Refresh models" — remote providers only (local list from the DB). Reconciles
  // deprecated/removed models against the provider's live list.
  const getRefreshButton = () => {
    if (!currentProvider || currentProvider.provider_type === 'local') return null
    // Refresh mutates is_deprecated → gate on edit (matches the backend's
    // llm_models::edit on POST /refresh-models), not create.
    if (!canEditModels) return null
    const refreshing = Boolean(Stores.LlmProvider.refreshingModels[currentProvider.id])
    return (
      <Tooltip content="Refresh models from provider">
        <span className="inline-flex">
          <Button
            variant="ghost"
            size="icon"
            icon={<RefreshCw aria-hidden="true" />}
            loading={refreshing}
            disabled={refreshing}
            onClick={handleRefreshModels}
            aria-label="Refresh models from provider"
            data-testid="llm-models-refresh-btn"
          />
        </span>
      </Tooltip>
    )
  }

  const getExtra = () => (
    <Flex align="center" className="gap-1">
      {getRefreshButton()}
      {getAddButton()}
    </Flex>
  )

  // Return early if no provider
  if (!currentProvider) {
    return null
  }

  return (
    <Card title="Models" extra={getExtra()} data-testid="llm-models-section-card">
      {loading ? (
        <Loading />
      ) : llmModels.length === 0 ? (
        <div>
          <Empty description="No models yet" data-testid="llm-models-empty" />
        </div>
      ) : (
        <div>
          {llmModels.map((llmModel, index: number) => (
            <div key={llmModel.id}>
              <div className="flex items-start gap-3 flex-wrap">
                {/* Model Info */}
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-2 flex-wrap">
                    <div className={'flex-1 min-w-48'}>
                      <Text className="font-medium">
                        {llmModel.display_name}
                      </Text>
                      {llmModel.is_deprecated && (
                        <Tooltip content="This model is no longer offered by the provider (or is deprecated). Calls to it may fail.">
                          <Badge
                            tone="warning"
                            className="ms-2 align-middle"
                            data-testid={`llm-model-deprecated-badge-${llmModel.id}`}
                          >
                            Deprecated
                          </Badge>
                        </Tooltip>
                      )}
                    </div>
                    <div className={'flex flex-wrap gap-1 items-center justify-end'}>
                      {getLlmModelActions(llmModel)}
                    </div>
                  </div>

                  <div className="space-y-1">
                    <Text type="secondary" className="text-sm block">
                      Model ID: {llmModel.name}
                    </Text>
                    {llmModel.is_active && llmModel.port && (
                      <Text type="secondary" className="text-sm block">
                        Running on:{' '}
                        <a
                          href={`http://127.0.0.1:${llmModel.port}`}
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          http://127.0.0.1:{llmModel.port}
                        </a>
                      </Text>
                    )}
                    {llmModel.description && (
                      <Text type="secondary" className="block">
                        {llmModel.description}
                      </Text>
                    )}
                    {llmModel.capabilities && (
                      <Flex wrap className="gap-3 pt-1 flex-wrap">
                        {llmModel.capabilities.vision && (
                          <Text type="secondary" className="text-sm">
                            👁️ Vision
                          </Text>
                        )}
                        {llmModel.capabilities.audio && (
                          <Text type="secondary" className="text-sm">
                            🎵 Audio
                          </Text>
                        )}
                        {llmModel.capabilities.tools && (
                          <Text type="secondary" className="text-sm">
                            🔧 Tools
                          </Text>
                        )}
                        {llmModel.capabilities.code_interpreter && (
                          <Text type="secondary" className="text-sm">
                            💻 Code
                          </Text>
                        )}
                        {llmModel.capabilities.chat && (
                          <Text type="secondary" className="text-sm">
                            💬 Chat
                          </Text>
                        )}
                        {llmModel.capabilities.text_embedding && (
                          <Text type="secondary" className="text-sm">
                            🔍 Embedding
                          </Text>
                        )}
                        {llmModel.capabilities.image_generator && (
                          <Text type="secondary" className="text-sm">
                            🎨 Image Gen
                          </Text>
                        )}
                      </Flex>
                    )}
                  </div>
                </div>
              </div>
              {index < llmModels.length - 1 && <Separator className="my-4" />}
            </div>
          ))}
        </div>
      )}
    </Card>
  )
}
