import { useState, useMemo, lazy, Suspense, ChangeEvent } from 'react'
import { Button, Input, MultiSelect, Combobox, Text, ErrorState, Empty } from '@ziee/kit'
import { Loading } from '@/core/components/Loading'
import { Plug, Search, Eraser } from 'lucide-react'
import { McpServerHubCard } from '@/modules/hub/modules/mcp/components/McpServerHubCard'
import { compatOf } from '@/modules/hub/stores/hub-catalog-store'
import { HubCatalog } from '@/modules/hub/stores/hub-catalog-store'
import { HubMcpServers } from '@/modules/hub/modules/mcp/stores/hub-mcp-servers-store'
const McpServerDrawer = lazy(() =>
  import('@/modules/mcp/components/common/McpServerDrawer').then(m => ({
    default: m.McpServerDrawer,
  })),
)

export function McpServersHubTab() {
  const { servers, loading, error } = HubMcpServers // Auto-loads via __init__
  const catalog = HubCatalog.catalog
  const serverVersion = HubCatalog.serverVersion
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
      <Loading tip="Loading MCP servers..." label="Loading" />
    )
  }

  // Show error state
  if (error && servers.length === 0) {
    return (
      <ErrorState
        variant="page"
        resource="hub MCP servers"
        description="The hub catalog couldn't be loaded. Check your connection and try again."
        details={error}
        onRetry={() => HubMcpServers.loadServers()}
        data-testid="hub-mcp-error"
      />
    )
  }

  return (
    <div className="mcp-servers-hub-tab flex flex-col gap-3 h-full overflow-hidden pt-1">
      {/* Search and Filters */}
      <div className="px-3">
        <div className="flex gap-2 flex-wrap">
          <Input
            data-testid="hub-mcp-search-input"
            placeholder="Search MCP servers..."
            prefix={<Search />}
            value={searchTerm}
            onChange={(e: ChangeEvent<HTMLInputElement>) => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
            aria-label="Search MCP servers"
          />

          <MultiSelect
            data-testid="hub-mcp-tags-multiselect"
            placeholder="Filter by tags"
            searchPlaceholder="Search tags..."
            emptyText="No tags found"
            value={selectedTags}
            onChange={setSelectedTags}
            className="flex-1"
            options={serverTags.map(tag => ({
              value: tag,
              label: tag,
            }))}
            aria-label="Filter by tags"
            removeLabel={(label) => `Remove ${label}`}
          />

          <Combobox
            data-testid="hub-mcp-sort-combobox"
            placeholder="Sort by"
            value={sortBy}
            onChange={(value: string) => setSortBy(value)}
            className="flex-1"
            options={[
              { value: 'popular', label: 'Popular' },
              { value: 'name', label: 'Name' },
            ]}
            aria-label="Sort MCP servers"
            searchPlaceholder="Search sort options"
            emptyText="No options found"
          />
        </div>

        {(searchTerm || selectedTags.length > 0) && (
          <div className="flex items-center gap-2 mt-2">
            <Text tone="secondary" className="text-xs">
              Filters active:{' '}
              {[
                searchTerm && 'search',
                selectedTags.length > 0 && `${selectedTags.length} tags`,
              ]
                .filter(Boolean)
                .join(', ')}
            </Text>
            <Button
              size="default"
              variant="ghost"
              icon={<Eraser />}
              onClick={clearAllFilters}
              aria-label="Clear all filters"
              data-testid="hub-mcp-clear-filters-btn"
            >
              Clear all
            </Button>
          </div>
        )}
      </div>

      {/* Servers List — incompatible items hidden entirely. */}
      <div className="flex-1 overflow-auto p-3">
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
              {visible.length === 0 &&
                (searchTerm || selectedTags.length > 0 ? (
                  <Empty
                    data-testid="hub-mcp-empty"
                    icon={<Plug />}
                    title="No MCP servers match your search"
                    description="Try a different search term or clear the active filters."
                  >
                    <Button
                      variant="outline"
                      icon={<Eraser />}
                      onClick={clearAllFilters}
                      data-testid="hub-mcp-empty-clear-btn"
                    >
                      Clear filters
                    </Button>
                  </Empty>
                ) : (
                  <Empty
                    data-testid="hub-mcp-empty"
                    icon={<Plug />}
                    title="No MCP servers yet"
                    description="The hub catalog has no MCP servers to show right now — check back after a hub refresh."
                  />
                ))}
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
