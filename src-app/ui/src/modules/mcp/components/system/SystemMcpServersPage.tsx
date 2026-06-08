import { Button, Card, Flex, Input, Pagination, Select, Typography } from 'antd'
import { PlusOutlined, SearchOutlined, ClearOutlined } from '@ant-design/icons'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { McpServerCard } from '@/modules/mcp/components/common/McpServerCard'
import { McpServerDrawer } from '@/modules/mcp/components/common/McpServerDrawer'
import { McpServerGroupsAssignmentCard } from '@/modules/mcp/components/system/McpServerGroupsAssignmentCard'

const { Text } = Typography

export function SystemMcpServersPage() {
  const {
    systemServers,
    systemServersLoading,
    systemServersTotal,
    systemServersPage,
    systemServersPageSize,
    searchTerm,
    statusFilter,
  } = Stores.SystemMcpServer
  const setSearchTerm = Stores.SystemMcpServer.setSearchTerm
  const setStatusFilter = Stores.SystemMcpServer.setStatusFilter

  const clearAllFilters = () => {
    setSearchTerm('')
    setStatusFilter('all')
  }

  const handlePageChange = (page: number, size?: number) => {
    const nextSize = size || systemServersPageSize
    // Reset to page 1 when the user changes page size — matches
    // UsersSettings / UserGroupsSettings behavior.
    const nextPage = size && size !== systemServersPageSize ? 1 : page
    Stores.SystemMcpServer.loadSystemServers(nextPage, nextSize)
  }

  const handleCreateServer = () => {
    Stores.McpServerDrawer.openMcpServerDrawer(undefined, 'create-system')
  }

  // Server-side filtering — `systemServers` already reflects
  // searchTerm + statusFilter via the store setters. Sort dropped:
  // backend's default ORDER BY display_name ASC covers it.
  //
  // The backend excludes only the zero-config `files`/`memory` built-ins
  // server-side; configurable built-ins (filesystem / fetch / code_sandbox)
  // stay visible + editable, so no client-side filtering is needed here.
  const filteredServers = systemServers

  return (
    <SettingsPageContainer
      title="System MCP Servers"
      subtitle="Manage Model Context Protocol servers across the system"
    >
      <div className="flex flex-col gap-3 h-full">
        {systemServersLoading && (
          <Text type="secondary">Loading system servers...</Text>
        )}
        {/* Search and Filters */}
        <div className="flex gap-2 flex-wrap">
          <Input
            placeholder="Search servers..."
            prefix={<SearchOutlined />}
            value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
            aria-label="Search system MCP servers"
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
            ]}
          />
          <Can permission={Permissions.McpServersAdminCreate}>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={handleCreateServer}
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
            <Card
              key={server.id}
              classNames={{ body: '!p-0' }}
              className="overflow-hidden"
              data-server-id={server.id}
              data-server-name={server.display_name}
            >
              <McpServerCard server={server} isEditable={true} bordered={false} />
              <McpServerGroupsAssignmentCard serverId={server.id} />
            </Card>
          ))}
        </div>

        {filteredServers.length === 0 && (
          <div className="text-center py-12">
            <Text type="secondary">
              {searchTerm || statusFilter !== 'all'
                ? 'No servers match your search criteria'
                : 'No system MCP servers configured'}
            </Text>
          </div>
        )}

        {systemServersTotal > 0 && (
          <Flex justify="end">
            <Pagination
              current={systemServersPage}
              total={systemServersTotal}
              pageSize={systemServersPageSize}
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

        {/* Drawer */}
        <McpServerDrawer />
      </div>
    </SettingsPageContainer>
  )
}
