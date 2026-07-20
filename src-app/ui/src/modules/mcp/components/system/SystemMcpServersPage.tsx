import { Plus, Search, Eraser } from 'lucide-react'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Stores } from '@ziee/framework/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import { McpServerCard } from '@/modules/mcp/components/common/McpServerCard'
import { McpServerDrawer } from '@/modules/mcp/components/common/McpServerDrawer'
import { McpServerGroupsAssignmentCard } from '@/modules/mcp/components/system/McpServerGroupsAssignmentCard'
import { McpUserPolicyCard } from '@/modules/mcp/components/system/McpUserPolicyCard'
import { Button, Card, ErrorState, Flex, Text, Input, Select, Tabs } from '@ziee/kit'
import { ListPagination } from '@/components/common/ListPagination'

export function SystemMcpServersPage() {
  const {
    systemServers,
    systemServersLoading,
    systemServersError,
    systemServersTotal,
    systemServersPage,
    systemServersPageSize,
    searchTerm,
    statusFilter,
  } = Stores.SystemMcpServer
  const setSearchTerm = Stores.SystemMcpServer.setSearchTerm
  const setStatusFilter = Stores.SystemMcpServer.setStatusFilter
  // Hoisted out of the .map() below: each Stores.X.<prop> read calls
  // useEffect + useStore under the hood (proxy in core/stores.ts).
  // Inside .map() the hook count becomes a function of
  // filteredServers.length — empty on first render, N on the second
  // → "Rendered more hooks than during the previous render."
  const { multiUserMode } = Stores.AppMode

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
      // On single-admin desktop (`!multiUserMode`) the user MCP page
      // is hidden, so the qualifier "System" is redundant — this IS
      // the MCP page. Drop the subtitle for the same reason: it
      // mentions "across the system", which echoes the user/system
      // split that doesn't exist there.
      title={multiUserMode ? 'System MCP Servers' : 'MCP Servers'}
      subtitle={
        multiUserMode
          ? 'Manage Model Context Protocol servers across the system'
          : undefined
      }
    >
      <div className="flex flex-col gap-3">
        <Tabs
          defaultValue="servers"
          data-testid="mcp-system-tabs"
          items={[
            {
              key: 'servers',
              label: 'Servers',
              children: (
                <div className="flex flex-col gap-3">
        {systemServersLoading && !systemServersError && (
          <Text type="secondary">Loading system servers...</Text>
        )}
        {/* Search and Filters */}
        <div className="flex gap-2 flex-wrap">
          <Input
            placeholder="Search servers..."
            prefix={<Search />}
            value={searchTerm}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => setSearchTerm(e.target.value)}
            allowClear
            // The kit Input wrapper is `w-full`. Below sm keep that (search
            // takes its own full row; Select + Add wrap beneath). From sm up,
            // drop the width and become flex-1 (grow + basis-0) so search,
            // filter and Add share one row with the search input flexing.
            className="grow basis-full sm:basis-0 sm:w-auto min-w-0"
            aria-label="Search system MCP servers"
            data-testid="mcp-system-search-input"
          />
          <Select
            placeholder="Filter by status"
            value={statusFilter}
            onChange={setStatusFilter}
            aria-label="Filter servers by status"
            className="min-w-[150px]"
            allowClear
            clearLabel="Clear status filter"
            data-testid="mcp-system-status-select"
            options={[
              { label: 'All Servers', value: 'all' },
              { label: 'Enabled', value: 'enabled' },
              { label: 'Disabled', value: 'disabled' },
            ]}
          />
          <Can permission={Permissions.McpServersAdminCreate}>
            <Button
              variant="default"
              icon={<Plus />}
              onClick={handleCreateServer}
              data-testid="mcp-system-add-btn"
            >
              Add Server
            </Button>
          </Can>
        </div>

        {(searchTerm || statusFilter !== 'all') && (
          <Flex align="center" className="gap-2">
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
              data-testid="mcp-system-clear-filters-btn"
            >
              Clear all
            </Button>
          </Flex>
        )}

        {/* Servers List. The per-row GroupsAssignmentCard is hidden on
            single-admin desktop (Stores.AppMode.multiUserMode=false)
            because there are no user groups to assign to there. */}
        <div className="flex flex-col gap-3">
          {filteredServers.map(server => (
            <Card
              key={server.id}
              className="overflow-hidden"
              data-server-id={server.id}
              data-server-name={server.display_name}
              data-testid={`mcp-system-server-card-${server.id}`}
            >
              <div className="!p-0">
                <McpServerCard server={server} isEditable={true} bordered={false} />
                {multiUserMode && (
                  <McpServerGroupsAssignmentCard serverId={server.id} />
                )}
              </div>
            </Card>
          ))}
        </div>

        {systemServersError && filteredServers.length === 0 ? (
          <ErrorState
            resource="MCP servers"
            description="Something went wrong while loading system MCP servers."
            details={systemServersError}
            onRetry={() =>
              Stores.SystemMcpServer.loadSystemServers(
                systemServersPage,
                systemServersPageSize,
              )
            }
            data-testid="mcp-system-error"
          />
        ) : (
          filteredServers.length === 0 && (
            <div className="text-center py-12" data-testid="mcp-system-empty">
              <Text type="secondary">
                {searchTerm || statusFilter !== 'all'
                  ? 'No servers match your search criteria'
                  : 'No system MCP servers configured'}
              </Text>
            </div>
          )
        )}

        {systemServersTotal > 0 && (
          <ListPagination
          data-testid="mcp-system-pagination"
          current={systemServersPage}
          total={systemServersTotal}
          pageSize={systemServersPageSize as number}
          onChange={(page: number) => handlePageChange(page)}
          onPageSizeChange={(size: number) => handlePageChange(1, size)}
          itemNoun="servers"
          aria-label="System MCP servers pagination"
        />
        )}
                </div>
              ),
            },
            {
              key: 'policy',
              label: 'Policy',
              // Admin-only user policy card (self-hides on single-admin desktop).
              children: <McpUserPolicyCard />,
            },
          ]}
        />

        {/* Drawer */}
        <McpServerDrawer />
      </div>
    </SettingsPageContainer>
  )
}
