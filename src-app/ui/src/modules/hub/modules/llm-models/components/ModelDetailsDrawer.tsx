import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Flex, Tag, Typography, Card } from 'antd'
import { EyeOutlined, ToolOutlined, MessageOutlined } from '@ant-design/icons'
import type { HubModel } from '@/api-client/types'

const { Title, Text } = Typography

interface ModelDetailsDrawerProps {
  model: HubModel | null
  open: boolean
  onClose: () => void
}

export function ModelDetailsDrawer({
  model,
  open,
  onClose,
}: ModelDetailsDrawerProps) {
  if (!model) return null

  return (
    <Drawer title={model.display_name} open={open} onClose={onClose}>
      <Flex vertical className="gap-4">
        {/* Basic Info */}
        <div>
          <Title level={3} className="!m-0 !mb-2">
            {model.display_name}
          </Title>
          <Text type="secondary" className="text-xs">
            {model.name}
          </Text>
          {model.description && (
            <div className="mt-2">
              <Text type="secondary">{model.description}</Text>
            </div>
          )}
        </div>

        {/* Repository Information */}
        {(model.repository?.url || model.websiteUrl) && (
          <div>
            <Title level={5}>Links</Title>
            <Flex vertical className="gap-2">
              {model.repository?.url && (
                <Flex justify="space-between">
                  <Text type="secondary">Repository:</Text>
                  <Text className="text-right break-all">
                    <a
                      href={model.repository.url}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {model.repository.url}
                    </a>
                  </Text>
                </Flex>
              )}
              {model.websiteUrl && (
                <Flex justify="space-between">
                  <Text type="secondary">Website:</Text>
                  <Text className="text-right break-all">
                    <a
                      href={model.websiteUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {model.websiteUrl}
                    </a>
                  </Text>
                </Flex>
              )}
            </Flex>
          </div>
        )}

        {/* Sources — v2 Phase 7 replaces the flat repository_url /
            repository_path / main_filename / file_format / size_gb /
            quantization_options fields. Each source surfaces its
            registry, identifier, version pin, and per-quantization
            choices. */}
        {model.sources && model.sources.length > 0 && (
          <div>
            <Title level={5}>Sources</Title>
            <Flex vertical className="gap-3">
              {model.sources.map((source, idx) => (
                <Card key={idx} size="small">
                  <Flex vertical className="gap-2">
                    <Flex justify="space-between" align="center">
                      <Text strong>
                        {source.registryType} · {source.identifier}
                      </Text>
                      <Tag color="blue">
                        {source.fileFormat.toUpperCase()}
                      </Tag>
                    </Flex>
                    <Flex justify="space-between">
                      <Text type="secondary">Version:</Text>
                      <Text>{source.version}</Text>
                    </Flex>
                    {source.runtimeHint && (
                      <Flex justify="space-between">
                        <Text type="secondary">Runtime hint:</Text>
                        <Text>{source.runtimeHint}</Text>
                      </Flex>
                    )}
                    {source.contextLength && (
                      <Flex justify="space-between">
                        <Text type="secondary">Context length:</Text>
                        <Text>{source.contextLength}</Text>
                      </Flex>
                    )}
                    {source.quantizations.length > 0 && (
                      <div>
                        <Text type="secondary" className="text-xs">
                          Quantizations:
                        </Text>
                        <Flex vertical className="gap-1 mt-1">
                          {source.quantizations.map(q => (
                            <Flex
                              key={q.name}
                              justify="space-between"
                              align="center"
                            >
                              <div>
                                <Text strong>{q.name}</Text>
                                {q.isDefault && (
                                  <Tag color="geekblue" className="ml-2 text-xs">
                                    default
                                  </Tag>
                                )}
                                <br />
                                <Text type="secondary" className="text-xs">
                                  {q.mainFile}
                                </Text>
                              </div>
                              <Text>{q.sizeGb} GB</Text>
                            </Flex>
                          ))}
                        </Flex>
                      </div>
                    )}
                  </Flex>
                </Card>
              ))}
            </Flex>
          </div>
        )}

        {/* Model Details */}
        <div>
          <Title level={5}>Model Details</Title>
          <Flex vertical className="gap-2">
            {model.license && (
              <Flex justify="space-between">
                <Text type="secondary">License:</Text>
                <Text>{model.license}</Text>
              </Flex>
            )}
            {model.author && (
              <Flex justify="space-between">
                <Text type="secondary">Author:</Text>
                <Text>{model.author}</Text>
              </Flex>
            )}
          </Flex>
        </div>

        {/* Dependencies — v2 Phase 7 informational deps. */}
        {model.dependencies && model.dependencies.length > 0 && (
          <div>
            <Title level={5}>Works best with</Title>
            <Flex wrap className="gap-1">
              {model.dependencies.map(dep => {
                const leaf = dep.name.split('/').slice(-1)[0]
                return (
                  <Tag
                    key={`${dep.kind}-${dep.name}`}
                    color={dep.kind === 'model' ? 'cyan' : 'purple'}
                  >
                    {leaf} {dep.versionRange}
                  </Tag>
                )
              })}
            </Flex>
          </div>
        )}

        {/* Capabilities */}
        {model.capabilities && (
          <div>
            <Title level={5}>Capabilities</Title>
            <Flex wrap className="gap-2">
              {model.capabilities.vision && (
                <Tag color="purple" icon={<EyeOutlined />}>
                  Vision
                </Tag>
              )}
              {model.capabilities.tools && (
                <Tag color="blue" icon={<ToolOutlined />}>
                  Function Calling
                </Tag>
              )}
              {model.capabilities.chat && (
                <Tag color="cyan" icon={<MessageOutlined />}>
                  Chat
                </Tag>
              )}
            </Flex>
          </div>
        )}

        {/* Tags */}
        {model.tags && model.tags.length > 0 && (
          <div>
            <Title level={5}>Tags</Title>
            <Flex wrap className="gap-1">
              {model.tags.map(tag => (
                <Tag key={tag} color="default">
                  {tag}
                </Tag>
              ))}
            </Flex>
          </div>
        )}

        {/* Recommended Parameters */}
        {model.recommended_parameters &&
          Object.keys(model.recommended_parameters).length > 0 && (
            <div>
              <Title level={5}>Recommended Parameters</Title>
              <Card size="small">
                <pre className="text-xs overflow-auto m-0">
                  {JSON.stringify(model.recommended_parameters, null, 2)}
                </pre>
              </Card>
            </div>
          )}
      </Flex>
    </Drawer>
  )
}
