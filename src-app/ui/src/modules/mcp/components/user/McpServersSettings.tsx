import { useEffect } from 'react'
import {
  App,
  Button,
  Flex,
  Input,
  Pagination,
  Select,
  Spin,
  Typography,
} from 'antd'
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
  const {
    servers,
    loading,
    error,
    total: totalServers,
    currentPage: storePage,
    pageSize: storePageSize,
    searchTerm,
    statusFilter,
  } = Stores.McpServer
  const setSearchTerm = Stores.McpServer.setSearchTerm
  const setStatusFilter = Stores.McpServer.setStatusFilter

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

  const handlePageChange = (page: number, size?: number) => {
    const nextSize = size || storePageSize
    // Reset to page 1 when the user changes page size, so the new
    // window starts at the top — matches UsersSettings behavior.
    const nextPage = size && size !== storePageSize ? 1 : page
    Stores.McpServer.loadMcpServers(nextPage, nextSize)
  }

  // Server-side filtering — `servers` already reflects the
  // searchTerm + statusFilter pushed through the backend, so the
  // UI just renders what came back. Sort dropped: backend's
  // default ORDER BY (is_system ASC, display_name ASC) covers it.
  const filteredServers = servers

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

        {totalServers > 0 && (
          <Flex justify="end">
            <Pagination
              current={storePage}
              total={totalServers}
              pageSize={storePageSize}
              showSizeChanger
              showQuickJumper
              showTotal={(total, range) =>
                `${range[0]}-${range[1]} of ${total} servers`
              }
              onChange={handlePageChange}
              onShowSizeChange={handlePageChange}
              pageSizeOptions={['5', '10', '20', '50']}
            />
          </Flex>
        )}
      </div>

      {/* Drawer */}
      <McpServerDrawer />
    </SettingsPageContainer>
  )
}
