import {
  CloudDownloadOutlined,
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
} from '@ant-design/icons'
import {
  App,
  Button,
  Card,
  Divider,
  Empty,
  Flex,
  Popconfirm,
  Switch,
  Typography,
} from 'antd'
import { useState } from 'react'
import { Stores } from '@/core/stores'
import {
  deleteLlmRepository,
  testLlmRepositoryConnection,
  updateLlmRepository,
  llmRepositoryHasCredentials,
} from '../store'
import type { LlmRepository } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'
import { LlmRepositoryDrawer } from './LlmRepositoryDrawer'

const { Text } = Typography

export function LlmRepositorySettings() {
  const { message } = App.useApp()

  // Stores
  const { repositories, testing } = Stores.LlmRepository

  // Drawer state
  const [drawerOpen, setDrawerOpen] = useState(false)
  const [editingRepository, setEditingRepository] = useState<LlmRepository | null>(null)

  const testRepositoryConnection = async (repository: LlmRepository) => {
    // Check if repository has credentials configured
    if (!llmRepositoryHasCredentials(repository)) {
      message.warning(
        'Please configure authentication credentials for this repository first',
      )
      return
    }

    try {
      const testData = {
        name: repository.name,
        url: repository.url,
        auth_type: repository.auth_type,
        auth_config: repository.auth_config,
      }

      const result = await testLlmRepositoryConnection(testData)

      if (result.success) {
        message.success(
          result.message || `Connection to ${repository.name} successful!`,
        )
      } else {
        message.error(
          result.message || `Connection to ${repository.name} failed`,
        )
      }
    } catch (error: any) {
      console.error('Repository connection test failed:', error)
      message.error(error?.message || `Connection to ${repository.name} failed`)
    }
  }

  // Repository management functions
  const handleAddRepository = () => {
    setEditingRepository(null)
    setDrawerOpen(true)
  }

  const handleEditRepository = (repository: LlmRepository) => {
    setEditingRepository(repository)
    setDrawerOpen(true)
  }

  const handleCloseDrawer = () => {
    setDrawerOpen(false)
    setEditingRepository(null)
  }

  const handleDeleteRepository = async (repositoryId: string) => {
    // Don't allow deleting built-in repositories
    const repo = repositories.find(r => r.id === repositoryId)
    if (repo?.built_in) {
      message.warning('Built-in repositories cannot be deleted')
      return
    }

    try {
      await deleteLlmRepository(repositoryId)
      message.success('Repository removed successfully')
    } catch (error: any) {
      console.error('Failed to delete repository:', error)
      message.error(error?.message || 'Failed to delete repository')
    }
  }

  const handleToggleRepository = async (
    repositoryId: string,
    enabled: boolean,
  ) => {
    try {
      await updateLlmRepository(repositoryId, { enabled })
    } catch (error: any) {
      console.error('Failed to toggle repository:', error)
      message.error(error?.message || 'Failed to toggle repository')
    }
  }

  const getRepositoryActions = (repository: LlmRepository) => {
    const actions: React.ReactNode[] = []

    // Always include the enable/disable switch first
    actions.push(
      <Switch
        key="enable"
        className="!mr-2"
        checked={repository.enabled}
        onChange={checked => handleToggleRepository(repository.id, checked)}
      />,
    )

    actions.push(
      <Button
        key="test"
        type="text"
        icon={<CloudDownloadOutlined />}
        loading={testing}
        onClick={() => testRepositoryConnection(repository)}
      >
        Test
      </Button>,
    )

    actions.push(
      <Button
        key="edit"
        type="text"
        icon={<EditOutlined />}
        onClick={() => handleEditRepository(repository)}
      >
        Edit
      </Button>,
    )

    if (!repository.built_in) {
      actions.push(
        <Popconfirm
          key="delete"
          title="Are you sure?"
          onConfirm={() => handleDeleteRepository(repository.id)}
          okText="Delete"
          cancelText="Cancel"
          okButtonProps={{ danger: true }}
        >
          <Button
            type="text"
            danger
            icon={<DeleteOutlined />}
          >
            Delete
          </Button>
        </Popconfirm>,
      )
    }

    return actions.filter(Boolean)
  }

  return (
    <SettingsPageContainer
      title="LLM Repositories"
      subtitle="Manage your LLM model repositories and their authentication settings"
    >
      {/* Model Repositories */}
      <Card
        title={
          <Flex align="center" gap="middle">
            <CloudDownloadOutlined />
            <span>Model Repositories</span>
          </Flex>
        }
        extra={
          <Button
            type={'text'}
            icon={<PlusOutlined />}
            onClick={handleAddRepository}
          />
        }
      >
        <Flex className="flex-col gap-4">
          <div>
            {repositories.length === 0 ? (
              <Empty
                description="No repositories configured"
                image={
                  <CloudDownloadOutlined className="text-4xl opacity-50" />
                }
              >
                <Text type="secondary">Add a repository to get started</Text>
              </Empty>
            ) : (
              <div>
                {repositories.map((repository, index) => (
                  <div key={repository.id}>
                    <div className="flex items-start gap-3 flex-wrap">
                      {/* Repository Info */}
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-2 flex-wrap-reverse">
                          <div className="flex-1 min-w-48">
                            <Flex align="center" gap="small">
                              <Text className="font-medium">
                                {repository.name}
                              </Text>
                              {repository.built_in && (
                                <Text type="secondary" className="text-xs">
                                  (Built-in)
                                </Text>
                              )}
                              {!repository.enabled && (
                                <Text type="secondary" className="text-xs">
                                  (Disabled)
                                </Text>
                              )}
                            </Flex>
                          </div>
                          <div className="flex gap-1 items-center justify-end">
                            {getRepositoryActions(repository)}
                          </div>
                        </div>

                        <div className="space-y-1">
                          <Text type="secondary" className="block">
                            {repository.url}
                          </Text>
                          <Text type="secondary" className="text-xs block">
                            Authentication:{' '}
                            {repository.auth_type === 'none'
                              ? 'None'
                              : repository.auth_type === 'api_key'
                                ? 'API Key'
                                : repository.auth_type === 'basic_auth'
                                  ? 'Basic Auth'
                                  : repository.auth_type === 'bearer_token'
                                    ? 'Bearer Token'
                                    : repository.auth_type}
                          </Text>
                        </div>
                      </div>
                    </div>
                    {index < repositories.length - 1 && (
                      <Divider className="my-4" />
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        </Flex>
      </Card>

      <LlmRepositoryDrawer
        repository={editingRepository}
        open={drawerOpen}
        onClose={handleCloseDrawer}
      />
    </SettingsPageContainer>
  )
}
