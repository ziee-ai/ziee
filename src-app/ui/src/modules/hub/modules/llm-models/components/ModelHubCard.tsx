import { App, Card, Tag, Typography, Button, Flex, Select } from 'antd'
import {
  AppstoreOutlined,
  DownloadOutlined,
  EyeOutlined,
  FileTextOutlined,
  LockOutlined,
  MessageOutlined,
  PictureOutlined,
  SearchOutlined,
  ToolOutlined,
  UnlockOutlined,
} from '@ant-design/icons'
import {
  Permissions,
  type HubLocalProvider,
  type HubModel,
  type HubModelQuantizationOption,
} from '@/api-client/types'
import { useState } from 'react'
import { ModelDetailsDrawer } from '@/modules/hub/modules/llm-models/components/ModelDetailsDrawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'

const { Text } = Typography

interface ModelHubCardProps {
  model: HubModel
}

export function ModelHubCard({ model }: ModelHubCardProps) {
  const { message, modal } = App.useApp()
  const [showDetails, setShowDetails] = useState(false)
  const canDownload = usePermission(Permissions.HubModelsCreate)

  const { localProviders } = Stores.HubModels
  const { downloads } = Stores.LlmModelDownload

  // Find active download for this model
  const activeDownload = downloads.find(
    download =>
      download.request_data.repository_path === model.repository_path &&
      (download.status === 'downloading' || download.status === 'pending'),
  )

  const isModelBeingDownloaded = !!activeDownload

  // Check if this hub model has been downloaded (system-wide tracking via hub)
  const isModelDownloaded = (model.created_ids?.length ?? 0) > 0

  const handleDownload = async () => {
    if (localProviders.length === 0) {
      message.error(
        `No local provider found. Please ask an administrator to configure a local provider.`,
      )
      return
    }

    let provider: HubLocalProvider = localProviders[0]
    let selectedQuantization: HubModelQuantizationOption | undefined = undefined

    // Handle quantization options selection
    if (model.quantization_options && model.quantization_options.length > 1) {
      selectedQuantization = model.quantization_options[0]

      await new Promise<void>(resolve => {
        let m = modal.info({
          icon: null,
          footer: null,
          title: 'Select Quantization',
          closable: false,
          onCancel: () => {
            selectedQuantization = undefined
            resolve()
          },
          content: (
            <div className="flex flex-col gap-2">
              <Text>
                Multiple quantization options available. Please select one:
              </Text>
              <Select
                options={model.quantization_options!.map(option => ({
                  label: (
                    <div className="flex flex-col">
                      <Text strong>{option.name.toUpperCase()}</Text>
                      <Text type="secondary" className="text-xs">
                        Main file: {option.main_filename}
                      </Text>
                    </div>
                  ),
                  value: option.name,
                }))}
                defaultValue={model.quantization_options![0].name}
                onChange={value => {
                  selectedQuantization = model.quantization_options!.find(
                    opt => opt.name === value,
                  )
                }}
                placeholder="Select quantization"
                optionRender={option => option.label}
                labelRender={props => (
                  <Text strong>{props.value?.toString().toUpperCase()}</Text>
                )}
              />
              <Flex className={'gap-2 w-full justify-end'}>
                <Button
                  onClick={() => {
                    selectedQuantization = undefined
                    m.destroy()
                    resolve()
                  }}
                >
                  Cancel
                </Button>
                <Button
                  type="primary"
                  onClick={() => {
                    resolve()
                    m.destroy()
                  }}
                >
                  Continue
                </Button>
              </Flex>
            </div>
          ),
        })
      })

      if (!selectedQuantization) {
        return
      }
    } else if (
      model.quantization_options &&
      model.quantization_options.length === 1
    ) {
      selectedQuantization = model.quantization_options[0]
    }

    if (localProviders.length > 1) {
      await new Promise<void>(resolve => {
        let m = modal.info({
          icon: null,
          footer: null,
          title: 'Select Local Provider',
          closable: false,
          onCancel: () => {
            provider = undefined as any
            resolve()
          },
          content: (
            <div className="flex flex-col gap-2">
              <Text>
                Multiple local providers found. Please select one to download
                the model:
              </Text>
              <Select
                options={localProviders.map(p => ({
                  label: p.name,
                  value: p.id,
                }))}
                defaultValue={localProviders[0].id}
                onChange={value => {
                  provider = localProviders.find(p => p.id === value)!
                }}
                placeholder="Select a provider"
              />
              <Flex className={'gap-2 w-full justify-end'}>
                <Button
                  onClick={() => {
                    provider = undefined as any
                    m.destroy()
                    resolve()
                  }}
                >
                  Cancel
                </Button>
                <Button
                  type="primary"
                  onClick={() => {
                    resolve()
                    m.destroy()
                  }}
                >
                  Continue
                </Button>
              </Flex>
            </div>
          ),
        })
      })
    }

    if (!provider) {
      return
    }

    try {
      const display_name = selectedQuantization
        ? `${model.display_name} (${selectedQuantization.name.toUpperCase()})`
        : model.display_name

      await Stores.HubModels.downloadModelFromHub(
        model.id,
        provider.id,
        display_name,
        selectedQuantization?.name,
      )

      message.success(
        `Download started for ${model.display_name}. You can monitor the progress in the download view.`,
      )
    } catch (error: any) {
      console.error('Failed to start model download:', error)
      message.error(
        `Failed to start download for ${model.display_name}: ${error.message || 'Unknown error'}`,
      )
    }
  }

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
        onClick={() => setShowDetails(true)}
        data-model-id={model.id}
        data-testid={`hub-model-card-${model.id}`}
      >
        <div className="flex items-start gap-3 flex-wrap">
          {/* Model Info */}
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <div className="flex-1 min-w-48">
                <Flex className="gap-2 items-center">
                  <AppstoreOutlined />
                  <Text className="font-medium cursor-pointer">
                    {model.display_name}
                  </Text>
                  {model.public ? (
                    <Tag color="green" icon={<UnlockOutlined />}>
                      Public
                    </Tag>
                  ) : (
                    <Tag color="red" icon={<LockOutlined />}>
                      Private
                    </Tag>
                  )}
                  {isModelBeingDownloaded && (
                    <Tag color="blue">Downloading...</Tag>
                  )}
                  {isModelDownloaded && (
                    <Tag color="geekblue-inverse">Downloaded</Tag>
                  )}
                  {model.auth_required && (
                    <Tag color="orange" icon={<LockOutlined />}>
                      Auth Required
                    </Tag>
                  )}
                </Flex>
              </div>
              <div className="flex gap-1 items-center justify-end">
                <Button
                  icon={<FileTextOutlined />}
                  onClick={e => {
                    e.stopPropagation()
                    const readmeUrl = `${model.repository_url}/${model.repository_path}/blob/main/README.md`
                    window.open(readmeUrl, '_blank')
                  }}
                >
                  README
                </Button>
                {canDownload && (
                  <Button
                    type="primary"
                    icon={<DownloadOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      handleDownload()
                    }}
                    disabled={isModelBeingDownloaded}
                    loading={isModelBeingDownloaded}
                  >
                    Download
                  </Button>
                )}
              </div>
            </div>

            <div>
              {model.description && (
                <Text type="secondary" className="text-sm mb-2 block">
                  {model.description}
                </Text>
              )}

              {/* Capabilities */}
              {model.capabilities && (
                <div className="mb-2">
                  <Text type="secondary" className="text-xs mr-2">
                    Capabilities:
                  </Text>
                  <Flex
                    wrap
                    className="gap-1"
                    style={{ display: 'inline-flex' }}
                  >
                    {model.capabilities.vision && (
                      <Tag
                        color="purple"
                        icon={<EyeOutlined />}
                        className="text-xs"
                      >
                        Vision
                      </Tag>
                    )}
                    {model.capabilities.tools && (
                      <Tag
                        color="blue"
                        icon={<ToolOutlined />}
                        className="text-xs"
                      >
                        Tools
                      </Tag>
                    )}
                    {model.capabilities.code_interpreter && (
                      <Tag
                        color="orange"
                        icon={<AppstoreOutlined />}
                        className="text-xs"
                      >
                        Code
                      </Tag>
                    )}
                    {model.capabilities.chat && (
                      <Tag
                        color="green"
                        icon={<MessageOutlined />}
                        className="text-xs"
                      >
                        Chat
                      </Tag>
                    )}
                    {model.capabilities.text_embedding && (
                      <Tag
                        color="cyan"
                        icon={<SearchOutlined />}
                        className="text-xs"
                      >
                        Embedding
                      </Tag>
                    )}
                    {model.capabilities.image_generator && (
                      <Tag
                        color="magenta"
                        icon={<PictureOutlined />}
                        className="text-xs"
                      >
                        Image Gen
                      </Tag>
                    )}
                  </Flex>
                </div>
              )}

              {/* Tags */}
              {model.tags && model.tags.length > 0 && (
                <div className="mb-2">
                  <Text type="secondary" className="text-xs mr-2">
                    Tags:
                  </Text>
                  <Flex
                    wrap
                    className="gap-1"
                    style={{ display: 'inline-flex' }}
                  >
                    {model.tags.map(tag => (
                      <Tag key={tag} color="default" className="text-xs">
                        {tag}
                      </Tag>
                    ))}
                  </Flex>
                </div>
              )}

              {/* Metadata */}
              <div className="mb-2">
                <Flex wrap className="gap-x-4 text-xs">
                  <span>
                    <Text type="secondary" className="text-xs">
                      Size:
                    </Text>{' '}
                    {model.size_gb} GB
                  </span>
                  <span>
                    <Text type="secondary" className="text-xs">
                      Format:
                    </Text>{' '}
                    {model.file_format?.toUpperCase()}
                  </span>
                  {model.license && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        License:
                      </Text>{' '}
                      {model.license}
                    </span>
                  )}
                  {model.author && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Author:
                      </Text>{' '}
                      {model.author}
                    </span>
                  )}
                </Flex>
              </div>
            </div>
          </div>
        </div>
      </Card>

      <ModelDetailsDrawer
        model={showDetails ? model : null}
        open={showDetails}
        onClose={() => setShowDetails(false)}
      />
    </>
  )
}
