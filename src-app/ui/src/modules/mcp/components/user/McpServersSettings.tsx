import { useEffect } from 'react'
import {
  Button,
  Input,
  Select,
  Text,
} from '@/components/ui'
import { Search, Eraser } from 'lucide-react'
import { AddButton } from '@/modules/settings/components/AddButton'
import { Loading } from '@/core/components/Loading'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { McpServerCard } from '@/modules/mcp/components/common/McpServerCard'
import { McpServerDrawer } from '@/modules/mcp/components/common/McpServerDrawer'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { message } from '@/components/ui'
import { ListPagination } from '@/components/common/ListPagination'

export function McpServersSettings() {
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

  // Subscribe to the policy state property (not the function
  // accessor) so this component re-renders when the admin saves a
  // new policy and the Add button + empty-state copy update without
  // a page reload.
  const { policy: mcpUserPolicy } = Stores.McpUserPolicy
  const policyAllowsAdd =
    (mcpUserPolicy?.allowed_transports?.length ?? 0) > 0

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
        <Loading tip="Loading MCP servers..." />
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
              data-testid="mcp-settings-retry-btn"
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
            prefix={<Search />}
            value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
            aria-label="Search MCP servers"
            data-testid="mcp-settings-search-input"
          />
          <Select
            placeholder="Filter by status"
            value={statusFilter}
            onChange={setStatusFilter}
            className="min-w-[150px]"
            aria-label="Filter servers by status"
            data-testid="mcp-settings-status-select"
            options={[
              { label: 'All Servers', value: 'all' },
              { label: 'Enabled', value: 'enabled' },
              { label: 'Disabled', value: 'disabled' },
              { label: 'System', value: 'system' },
              { label: 'User', value: 'user' },
            ]}
          />
          <Can permission={Permissions.McpServersCreate}>
            {/* Hidden when admin policy disallows ALL user transports —
                the backend would 422 the create regardless. Surfaces
                the right empty-state copy below instead. */}
            {policyAllowsAdd && (
              <AddButton
                label="Add server"
                onClick={handleAddServer}
                data-testid="mcp-settings-add-btn"
              />
            )}
          </Can>
        </div>

        {(searchTerm || statusFilter !== 'all') && (
          <div className="flex items-center gap-2">
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
              size="default"
              variant="ghost"
              icon={<Eraser />}
              onClick={clearAllFilters}
              data-testid="mcp-settings-clear-filters-btn"
            >
              Clear all
            </Button>
          </div>
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
          <div className="text-center py-12" data-testid="mcp-settings-empty">
            <Text type="secondary">
              {searchTerm || statusFilter !== 'all'
                ? 'No servers match your search criteria'
                : !policyAllowsAdd
                  ? 'Your administrator has not enabled adding MCP servers.'
                  : 'No MCP servers configured'}
            </Text>
          </div>
        )}

        {totalServers > 0 && (
          <ListPagination
          data-testid="mcp-settings-pagination"
          current={storePage}
          total={totalServers}
          pageSize={storePageSize}
          onChange={handlePageChange}
          onPageSizeChange={(size: number) => handlePageChange(1, size)}
          itemNoun="servers"
          aria-label="Pagination"
        />
        )}
      </div>

      {/* Drawer */}
      <McpServerDrawer />
    </SettingsPageContainer>
  )
}
