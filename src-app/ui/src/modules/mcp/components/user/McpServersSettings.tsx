import { useEffect, useState } from 'react'
import { App, Button, Flex, Input, Select, Spin, Typography } from 'antd'
import { PlusOutlined, SearchOutlined, ClearOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { McpServerCard } from '@/modules/mcp/components/common/McpServerCard'
import { McpServerDrawer } from '@/modules/mcp/components/common/McpServerDrawer'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

const { Text } = Typography

export function McpServersSettings() {
  const { message } = App.useApp()
  const { servers, loading, error } = Stores.McpServer
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState('all')
  const [sortBy, setSortBy] = useState('created_at')

  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.McpServer.clearMcpError()
    }
  }, [error, message])

  const handleAddServer = () => {
    Stores.McpServerDrawer.openMcpServerDrawer(undefined, 'create')
  }

  const clearAllFilters = () => {
    setSearchTerm('')
    setStatusFilter('all')
  }

  // Filter and sort servers (both user and system servers with system tag)
  const filteredServers = servers
    .filter(server => {
      const matchesSearch =
        searchTerm === '' ||
        server.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        server.display_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        server.description?.toLowerCase().includes(searchTerm.toLowerCase())

      const matchesStatus =
        statusFilter === 'all' ||
        (statusFilter === 'enabled' && server.enabled) ||
        (statusFilter === 'disabled' && !server.enabled) ||
        (statusFilter === 'system' && server.is_system) ||
        (statusFilter === 'user' && !server.is_system)

      return matchesSearch && matchesStatus
    })
    .sort((a, b) => {
      switch (sortBy) {
        case 'name':
          return a.display_name.localeCompare(b.display_name)
        case 'status':
          return Number(b.enabled) - Number(a.enabled)
        case 'type':
          return Number(b.is_system) - Number(a.is_system)
        case 'created_at':
          return new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        default:
          return 0
      }
    })

  // Show loading state
  if (loading && servers.length === 0) {
    return (
      <SettingsPageContainer
        title="MCP Servers"
        subtitle="Manage Model Context Protocol servers for enhanced tool capabilities"
      >
        <div className="flex justify-center items-center h-full">
          <Spin size="large" />
          <Text className="ml-4">Loading MCP servers...</Text>
        </div>
      </SettingsPageContainer>
    )
  }

  // Show error state
  if (error && servers.length === 0) {
    return (
      <SettingsPageContainer
        title="MCP Servers"
        subtitle="Manage Model Context Protocol servers for enhanced tool capabilities"
      >
        <div className="text-center py-12">
          <Text type="danger">Failed to load MCP servers: {error}</Text>
          <div className="mt-4">
            <Button
              onClick={() => {
                Stores.McpServer.loadMcpServers().catch((err: Error) => {
                  console.error('Failed to load MCP servers:', err)
                  message.error('Failed to load MCP servers')
                })
              }}
            >
              Retry
            </Button>
          </div>
        </div>
      </SettingsPageContainer>
    )
  }

  return (
    <SettingsPageContainer
      title="MCP Servers"
      subtitle="Manage Model Context Protocol servers for enhanced tool capabilities"
    >
      <div className="flex flex-col gap-3">
        {/* Search and Filters */}
        <div className="flex gap-2 flex-wrap">
          <Input
            placeholder="Search servers..."
            prefix={<SearchOutlined />}
            value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
            aria-label="Search MCP servers"
          />
          <Select
            placeholder="Filter by status"
            value={statusFilter}
            onChange={setStatusFilter}
            style={{ minWidth: 150 }}
            allowClear
            aria-label="Filter servers by status"
            options={[
              { label: 'All Servers', value: 'all' },
              { label: 'Enabled', value: 'enabled' },
              { label: 'Disabled', value: 'disabled' },
              { label: 'System', value: 'system' },
              { label: 'User', value: 'user' },
            ]}
          />
          <Select
            placeholder="Sort by"
            value={sortBy}
            onChange={setSortBy}
            style={{ minWidth: 120 }}
            aria-label="Sort servers"
            options={[
              { label: 'Date Added', value: 'created_at' },
              { label: 'Name', value: 'name' },
              { label: 'Status', value: 'status' },
              { label: 'Type', value: 'type' },
            ]}
          />
          <Can permission={Permissions.McpServersCreate}>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={handleAddServer}
            >
              Add Server
            </Button>
          </Can>
        </div>

        {(searchTerm || statusFilter !== 'all') && (
          <Flex align="center" gap={8}>
            <Text type="secondary" className="text-xs">
              Filters active:{' '}
              {[
                searchTerm && 'search',
                statusFilter !== 'all' && `status: ${statusFilter}`,
              ]
                .filter(Boolean)
                .join(', ')}
            </Text>
            <Button
              size="small"
              type="text"
              icon={<ClearOutlined />}
              onClick={clearAllFilters}
            >
              Clear all
            </Button>
          </Flex>
        )}

        {/* Servers List */}
        <div className="flex flex-col gap-3">
          {filteredServers.map(server => (
            <McpServerCard
              key={server.id}
              server={server}
              isEditable={!server.is_system}
            />
          ))}
        </div>

        {filteredServers.length === 0 && (
          <div className="text-center py-12">
            <Text type="secondary">
              {searchTerm || statusFilter !== 'all'
                ? 'No servers match your search criteria'
                : 'No MCP servers configured'}
            </Text>
          </div>
        )}
      </div>

      {/* Drawer */}
      <McpServerDrawer />
    </SettingsPageContainer>
  )
}
