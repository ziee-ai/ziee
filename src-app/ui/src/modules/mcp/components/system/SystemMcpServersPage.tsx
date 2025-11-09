import { useState } from 'react'
import { Button, Input, Select, Typography, Flex } from 'antd'
import { PlusOutlined, SearchOutlined, ClearOutlined } from '@ant-design/icons'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Stores } from '@/core/stores'
import { McpServerCard } from '../common/McpServerCard'
import { McpServerDrawer } from '../common/McpServerDrawer'
import { McpServerGroupsAssignmentCard } from './McpServerGroupsAssignmentCard'

const { Text } = Typography

export function SystemMcpServersPage() {
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<string>('all')
  const [sortBy, setSortBy] = useState('name')

  const { systemServers, systemServersLoading } = Stores.SystemMcpServer

  const clearAllFilters = () => {
    setSearchTerm('')
    setStatusFilter('all')
  }

  const handleCreateServer = () => {
    Stores.McpServerDrawer.openMcpServerDrawer(undefined, 'create-system')
  }

  // Filter and sort servers
  const filteredServers = systemServers
    .filter(server => {
      const matchesSearch =
        searchTerm === '' ||
        server.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        server.display_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        server.description?.toLowerCase().includes(searchTerm.toLowerCase())

      const matchesStatus =
        statusFilter === 'all' ||
        (statusFilter === 'enabled' && server.enabled) ||
        (statusFilter === 'disabled' && !server.enabled)

      return matchesSearch && matchesStatus
    })
    .sort((a, b) => {
      switch (sortBy) {
        case 'name':
          return a.display_name.localeCompare(b.display_name)
        case 'status':
          return Number(b.enabled) - Number(a.enabled)
        default:
          return 0
      }
    })

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
          <Select
            placeholder="Sort by"
            value={sortBy}
            onChange={setSortBy}
            style={{ minWidth: 120 }}
            aria-label="Sort servers"
            options={[
              { label: 'Name', value: 'name' },
              { label: 'Status', value: 'status' },
            ]}
          />
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={handleCreateServer}
          >
            Add Server
          </Button>
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
            <div key={server.id} className="flex flex-col gap-3">
              <McpServerCard
                server={server}
                isEditable={true}
              />
              <McpServerGroupsAssignmentCard serverId={server.id} />
            </div>
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

        {/* Drawer */}
        <McpServerDrawer />
      </div>
    </SettingsPageContainer>
  )
}
