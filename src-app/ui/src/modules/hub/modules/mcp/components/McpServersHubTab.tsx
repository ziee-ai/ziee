import { useState, useMemo, lazy, Suspense } from 'react'
import { Input, Select, Typography, Button } from 'antd'
import { Loading } from '@/core/components/Loading'
import { SearchOutlined, ClearOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { McpServerHubCard } from '@/modules/hub/modules/mcp/components/McpServerHubCard'
import { compatOf } from '@/modules/hub/stores/hub-catalog-store'
const McpServerDrawer = lazy(() =>
  import('@/modules/mcp/components/common/McpServerDrawer').then(m => ({
    default: m.McpServerDrawer,
  })),
)

const { Text } = Typography

export function McpServersHubTab() {
  const { servers, loading, error } = Stores.HubMcpServers // Auto-loads via __init__
  const catalog = Stores.HubCatalog.catalog
  const serverVersion = Stores.HubCatalog.serverVersion
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [sortBy, setSortBy] = useState('popular')

  const clearAllFilters = () => {
    setSearchTerm('')
    setSelectedTags([])
    setSortBy('popular')
  }

  // v2: catalog curation (tags, title, summary) lives on IndexItem.
  // Build a name → IndexItem map so the tab can search/filter against
  // it; the manifest body itself no longer carries display fields.
  const indexByName = useMemo(() => {
    const m = new Map(
      (catalog?.items ?? [])
        .filter(it => it.category === 'mcp-server')
        .map(it => [it.name, it]),
    )
    return m
  }, [catalog])

  const serverTags = useMemo(() => {
    const allTags = new Set<string>()
    indexByName.forEach(ix => {
      ix.tags?.forEach(tag => allTags.add(tag))
    })
    return Array.from(allTags).sort()
  }, [indexByName])

  const filteredServers = useMemo(() => {
    let filtered = servers

    if (searchTerm) {
      const search = searchTerm.toLowerCase()
      filtered = filtered.filter(s => {
        const ix = indexByName.get(s.name)
        const title = ix?.title ?? ''
        return (
          s.name.toLowerCase().includes(search) ||
          title.toLowerCase().includes(search) ||
          s.description?.toLowerCase().includes(search)
        )
      })
    }

    if (selectedTags.length > 0) {
      filtered = filtered.filter(s => {
        const tags = indexByName.get(s.name)?.tags ?? []
        return selectedTags.some(tag => tags.includes(tag))
      })
    }

    // Sort. "Popular" is dropped (no popularity_score on the strict
    // manifest); both modes fall through to alphabetical sort on the
    // reverse-DNS name.
    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === 'name' || sortBy === 'popular')
        return a.name.localeCompare(b.name)
      return 0
    })

    return sorted
  }, [servers, searchTerm, selectedTags, sortBy, indexByName])

  // Show loading state
  if (loading && servers.length === 0) {
    return (
      <Loading tip="Loading MCP servers..." />
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
            aria-label="Search MCP servers"
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
            aria-label="Filter by tags"
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
            aria-label="Sort MCP servers"
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
              aria-label="Clear all filters"
            >
              Clear all
            </Button>
          </div>
        )}
      </div>

      {/* Servers List — incompatible items hidden entirely. */}
      <div className="flex-1 overflow-auto px-3 pb-3">
        {(() => {
          const visible = filteredServers.filter(s => {
            const ix = indexByName.get(s.name)
            return !ix || compatOf(ix, serverVersion).status === 'ok'
          })
          return (
            <>
              <div className="flex flex-col gap-3">
                {visible.map(server => (
                  <McpServerHubCard key={server.name} server={server} />
                ))}
              </div>
              {visible.length === 0 && (
                <div className="text-center py-12">
                  <Text type="secondary">
                    {servers.length === 0
                      ? 'No MCP servers yet'
                      : 'No MCP servers match your search'}
                  </Text>
                </div>
              )}
            </>
          )
        })()}
      </div>

      {/* The McpServerDrawer is a global singleton — its state lives
          in the McpServerDrawer zustand store. Mounted here so the
          hub MCP "Install for me" / "Install for the system" buttons
          can open it without navigating away from the Hub. The same
          drawer is mounted on /settings/mcp-servers and
          /settings/mcp-admin; only one is ever visible at a time
          because the user can only be on one route. */}
      <Suspense fallback={null}>
        <McpServerDrawer />
      </Suspense>
    </div>
  )
}
