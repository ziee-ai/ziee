import { useEffect, useState } from 'react'
import { App, Button, Dropdown, Input, Typography } from 'antd'
import { PlusOutlined, RobotOutlined, SearchOutlined } from '@ant-design/icons'
import { PiSortAscending } from 'react-icons/pi'
import { Stores } from '@/modules/assistant/stores'
import { Can } from '@/core/permissions'
import { Permissions, type Assistant } from '@/api-client/types'
import { AssistantCard } from '@/modules/assistant/components/AssistantCard'
import { AssistantFormDrawer } from '@/modules/assistant/components/AssistantFormDrawer'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { useMainContentMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

const { Title, Text } = Typography

export function UserAssistantsPage() {
  const { message } = App.useApp()
  const pageMinSize = useMainContentMinSize()
  const [isSearchBoxVisible, setIsSearchBoxVisible] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [sortBy, setSortBy] = useState<'activity' | 'name' | 'created'>(
    'activity',
  )

  // Destructure store values
  const { assistants: assistantsMap, loading, error } = Stores.UserAssistants

  // Convert Map to Array
  const assistants = Array.from(assistantsMap.values())

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.UserAssistants.clearUserAssistantsStoreError()
    }
  }, [error, message])

  const handleCreate = () => {
    Stores.AssistantDrawer.openAssistantDrawer(null, false)
  }

  const handleEdit = (assistant: Assistant) => {
    Stores.AssistantDrawer.openAssistantDrawer(assistant, false)
  }

  const handleDelete = async (assistant: Assistant) => {
    try {
      await Stores.UserAssistants.deleteUserAssistant(assistant.id)
      message.success('Assistant deleted successfully')
    } catch (_error) {
      message.error('Failed to delete assistant')
    }
  }

  // Get filtered and sorted assistants
  const getFilteredAndSortedAssistants = () => {
    let filteredAssistants = assistants

    // Apply search filter
    if (searchQuery.trim()) {
      filteredAssistants = assistants.filter(
        assistant =>
          assistant.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          assistant.description
            ?.toLowerCase()
            .includes(searchQuery.toLowerCase()),
      )
    }

    // Sort assistants
    const sortedAssistants = [...filteredAssistants]
    switch (sortBy) {
      case 'activity':
        sortedAssistants.sort(
          (a, b) =>
            new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
        )
        break
      case 'name':
        sortedAssistants.sort((a, b) => a.name.localeCompare(b.name))
        break
      case 'created':
        sortedAssistants.sort(
          (a, b) =>
            new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
        )
        break
    }

    return sortedAssistants
  }

  const searchInputComponent = (
    <Input
      placeholder="Search assistants"
      prefix={<SearchOutlined />}
      className="w-full items-center justify-center flex-1 pr-1"
      value={searchQuery}
      onChange={e => setSearchQuery(e.target.value)}
      allowClear
    />
  )

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Page Header */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full">
          <Typography.Title level={4} className="!m-0 !leading-tight">
            Assistants
          </Typography.Title>
          <div className="h-full flex items-center justify-between">
            {!pageMinSize.xs ? (
              <div className="pr-1">{searchInputComponent}</div>
            ) : (
              <Button
                type={isSearchBoxVisible ? 'primary' : 'text'}
                icon={<SearchOutlined aria-hidden="true" />}
                style={{
                  fontSize: '18px',
                }}
                onClick={() => setIsSearchBoxVisible(!isSearchBoxVisible)}
                aria-label="Search assistants"
              />
            )}
            <div className="flex gap-0">
              <Dropdown
                menu={{
                  items: [
                    {
                      key: 'activity',
                      label: 'Activity',
                      onClick: () => setSortBy('activity'),
                    },
                    {
                      key: 'name',
                      label: 'Name',
                      onClick: () => setSortBy('name'),
                    },
                    {
                      key: 'created',
                      label: 'Created',
                      onClick: () => setSortBy('created'),
                    },
                  ],
                  selectedKeys: [sortBy],
                }}
                trigger={['click']}
              >
                <Button
                  type="text"
                  icon={<PiSortAscending aria-hidden="true" />}
                  style={{
                    fontSize: '20px',
                  }}
                  aria-label="Sort assistants"
                />
              </Dropdown>
              <Can permission={Permissions.AssistantsCreate}>
                <Button
                  type="text"
                  icon={<PlusOutlined aria-hidden="true" />}
                  onClick={handleCreate}
                  style={{
                    fontSize: '16px',
                  }}
                  aria-label="Create assistant"
                />
              </Can>
            </div>
          </div>
        </div>
      </HeaderBarContainer>

      {/* Page Content */}
      <div className="flex-1 flex flex-col overflow-hidden items-center">
        {pageMinSize.xs && isSearchBoxVisible && (
          <div className="w-full max-w-96 px-3 pt-3">
            {searchInputComponent}
          </div>
        )}

        {/* Assistants Grid */}
        {(() => {
          const filteredAssistants = getFilteredAndSortedAssistants()

          if (filteredAssistants.length === 0) {
            return null
          }

          return (
            <div className="flex flex-1 flex-col w-full justify-center overflow-hidden">
              <div className="h-full flex flex-col overflow-y-auto">
                <div className="max-w-4xl flex flex-wrap gap-3 pt-3 w-full self-center px-3">
                  {filteredAssistants.map((assistant: Assistant) => (
                    <div key={assistant.id} className="min-w-70 flex-1">
                      <AssistantCard
                        assistant={assistant}
                        onEdit={handleEdit}
                        onDelete={() => handleDelete(assistant)}
                      />
                    </div>
                  ))}
                  {/* Placeholder divs for grid layout */}
                  <div className="min-w-70 flex-1"></div>
                  <div className="min-w-70 flex-1"></div>
                  <div className="min-w-70 flex-1"></div>
                </div>
              </div>
            </div>
          )
        })()}

        {/* Empty State */}
        {!loading && getFilteredAndSortedAssistants().length === 0 && (
          <div className="text-center py-12 m-auto">
            <RobotOutlined className="text-6xl mb-4" />
            <Title level={3} type="secondary">
              {searchQuery ? 'No assistants found' : 'No assistants yet'}
            </Title>
            <Text type="secondary" className="block mb-4">
              {searchQuery
                ? 'Try adjusting your search criteria'
                : 'Create your first assistant to get started'}
            </Text>
            {!searchQuery && (
              <Can permission={Permissions.AssistantsCreate}>
                <Button
                  type="primary"
                  icon={<PlusOutlined />}
                  onClick={handleCreate}
                >
                  Create Assistant
                </Button>
              </Can>
            )}
          </div>
        )}
      </div>

      <AssistantFormDrawer />
    </div>
  )
}
