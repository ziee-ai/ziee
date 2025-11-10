import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Flex, Tag, Typography, Card } from 'antd'
import {
  EyeOutlined,
  ToolOutlined,
  MessageOutlined,
} from '@ant-design/icons'
import type { HubModel } from '@/api-client/types'

const { Title, Text } = Typography

interface ModelDetailsDrawerProps {
  model: HubModel | null
  open: boolean
  onClose: () => void
}

export function ModelDetailsDrawer({ model, open, onClose }: ModelDetailsDrawerProps) {
  if (!model) return null

  return (
    <Drawer
      title={model.display_name}
      open={open}
      onClose={onClose}
    >
      <Flex vertical className="gap-4">
        {/* Basic Info */}
        <div>
          <Title level={3} className="!m-0 !mb-2">
            {model.display_name}
          </Title>
          {model.description && (
            <Text type="secondary">{model.description}</Text>
          )}
        </div>

        {/* Repository Information */}
        <div>
          <Title level={5}>Repository Information</Title>
          <Flex vertical className="gap-2">
            <Flex justify="space-between">
              <Text type="secondary">Repository URL:</Text>
              <Text className="text-right break-all">
                <a href={model.repository_url} target="_blank" rel="noopener noreferrer">
                  {model.repository_url}
                </a>
              </Text>
            </Flex>
            <Flex justify="space-between">
              <Text type="secondary">Repository Path:</Text>
              <Text className="text-right">{model.repository_path}</Text>
            </Flex>
            <Flex justify="space-between">
              <Text type="secondary">Main Filename:</Text>
              <Text className="text-right">{model.main_filename}</Text>
            </Flex>
          </Flex>
        </div>

        {/* Model Details */}
        <div>
          <Title level={5}>Model Details</Title>
          <Flex vertical className="gap-2">
            <Flex justify="space-between">
              <Text type="secondary">File Format:</Text>
              <Tag color="blue">{model.file_format?.toUpperCase()}</Tag>
            </Flex>
            <Flex justify="space-between">
              <Text type="secondary">Size:</Text>
              <Text>{model.size_gb} GB</Text>
            </Flex>
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
            {model.popularity_score && (
              <Flex justify="space-between">
                <Text type="secondary">Popularity Score:</Text>
                <Text>{model.popularity_score}</Text>
              </Flex>
            )}
          </Flex>
        </div>

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

        {/* Quantization Options */}
        {model.quantization_options &&
          model.quantization_options.length > 0 && (
            <div>
              <Title level={5}>Quantization Options</Title>
              <Flex vertical className="gap-2">
                {model.quantization_options.map(option => (
                  <Card key={option.name} size="small">
                    <Flex justify="space-between" align="center">
                      <div>
                        <Text strong>{option.name}</Text>
                        <br />
                        <Text type="secondary" className="text-xs">
                          {option.filename}
                        </Text>
                      </div>
                      <Text>{option.size_gb} GB</Text>
                    </Flex>
                  </Card>
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
