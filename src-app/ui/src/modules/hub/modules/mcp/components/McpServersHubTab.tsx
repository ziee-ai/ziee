import { useState, useMemo } from 'react'
import { Input, Select, Typography, Spin, Button } from 'antd'
import { SearchOutlined, ClearOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { McpServerHubCard } from './McpServerHubCard'

const { Text } = Typography

export function McpServersHubTab() {
  const { servers, loading, error } = Stores.HubMcpServers // Auto-loads via __init__
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [sortBy, setSortBy] = useState('popular')

  const clearAllFilters = () => {
    setSearchTerm('')
    setSelectedTags([])
  }

  // Extract unique tags
  const serverTags = useMemo(() => {
    const allTags = new Set<string>()
    servers.forEach(server => {
      server.tags?.forEach(tag => allTags.add(tag))
    })
    return Array.from(allTags).sort()
  }, [servers])

  // Filter and sort
  const filteredServers = useMemo(() => {
    let filtered = servers

    // Search
    if (searchTerm) {
      const search = searchTerm.toLowerCase()
      filtered = filtered.filter(s =>
        s.name.toLowerCase().includes(search) ||
        s.display_name.toLowerCase().includes(search) ||
        s.description?.toLowerCase().includes(search)
      )
    }

    // Tags
    if (selectedTags.length > 0) {
      filtered = filtered.filter(s =>
        selectedTags.some(tag => s.tags?.includes(tag))
      )
    }

    // Sort (create a copy to avoid mutating read-only array from store)
    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === 'popular') return (b.popularity_score || 0) - (a.popularity_score || 0)
      if (sortBy === 'name') return a.name.localeCompare(b.name)
      return 0
    })

    return sorted
  }, [servers, searchTerm, selectedTags, sortBy])

  // Show loading state
  if (loading && servers.length === 0) {
    return (
      <div className="flex justify-center items-center h-full">
        <Spin size="large" />
        <Text className="ml-4">Loading MCP servers...</Text>
      </div>
    )
  }

  // Show error state
  if (error && servers.length === 0) {
    return (
      <div className="text-center py-12">
        <Text type="danger">Failed to load MCP servers: {error}</Text>
        <div className="mt-4">
          <Button onClick={() => Stores.HubMcpServers.loadServers()}>
            Retry
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="mcp-servers-hub-tab flex flex-col gap-3 h-full overflow-hidden">
      {/* Search and Filters */}
      <div className="px-3">
        <div className="flex gap-2 flex-wrap">
          <Input
            placeholder="Search MCP servers..."
            prefix={<SearchOutlined />}
            value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
          />

          <Select
            mode="multiple"
            placeholder="Filter by tags"
            value={selectedTags}
            onChange={setSelectedTags}
            className="flex-1"
            allowClear
            maxTagCount="responsive"
            options={serverTags.map(tag => ({
              key: tag,
              value: tag,
              label: tag,
            }))}
            popupMatchSelectWidth={false}
          />

          <Select
            placeholder="Sort by"
            value={sortBy}
            onChange={setSortBy}
            className="flex-1"
            options={[
              { value: 'popular', label: 'Popular' },
              { value: 'name', label: 'Name' },
            ]}
            popupMatchSelectWidth={false}
          />
        </div>

        {(searchTerm || selectedTags.length > 0) && (
          <div className="flex items-center gap-2 mt-2">
            <Text type="secondary" className="text-xs">
              Filters active:{' '}
              {[
                searchTerm && 'search',
                selectedTags.length > 0 && `${selectedTags.length} tags`,
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
          </div>
        )}
      </div>

      {/* Servers List */}
      <div className="flex-1 overflow-auto px-3 pb-3">
        <div className="flex flex-col gap-3">
          {filteredServers.map(server => (
            <McpServerHubCard key={server.id} server={server} />
          ))}
        </div>

        {filteredServers.length === 0 && (
          <div className="text-center py-12">
            <Text type="secondary">No MCP servers found</Text>
          </div>
        )}
      </div>
    </div>
  )
}
