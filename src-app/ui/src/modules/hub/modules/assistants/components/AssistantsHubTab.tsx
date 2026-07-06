import { Search, Eraser } from 'lucide-react'
import { useState, useMemo } from 'react'
import { Combobox, MultiSelect, Text, Button, Input, ErrorState } from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { Stores } from '@/core/stores'
import { AssistantHubCard } from '@/modules/hub/modules/assistants/components/AssistantHubCard'
import { compatOf } from '@/modules/hub/stores/hub-catalog-store'

export function AssistantsHubTab() {
  const { assistants, loading, error } = Stores.HubAssistants // Auto-loads via __init__
  const catalog = Stores.HubCatalog.catalog
  const serverVersion = Stores.HubCatalog.serverVersion
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  // v2 Phase 7 dropped `popularity_score`; sort by name only.
  const [sortBy, setSortBy] = useState('name')

  const clearAllFilters = () => {
    setSearchTerm('')
    setSelectedTags([])
    setSortBy('name')
  }

  // Extract unique tags
  const assistantTags = useMemo(() => {
    const allTags = new Set<string>()
    assistants.forEach(assistant => {
      assistant.tags?.forEach(tag => allTags.add(tag))
    })
    return Array.from(allTags).sort()
  }, [assistants])

  // Filter and sort
  const filteredAssistants = useMemo(() => {
    let filtered = assistants

    // Search
    if (searchTerm) {
      const search = searchTerm.toLowerCase()
      filtered = filtered.filter(
        a =>
          a.name.toLowerCase().includes(search) ||
          a.display_name.toLowerCase().includes(search) ||
          a.description?.toLowerCase().includes(search),
      )
    }

    // Tags
    if (selectedTags.length > 0) {
      filtered = filtered.filter(a =>
        selectedTags.some(tag => a.tags?.includes(tag)),
      )
    }

    // Sort (create a copy to avoid mutating read-only array from store).
    // v2 Phase 7: `popularity_score` is gone; sort by canonical name
    // or display_name.
    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === 'display_name')
        return a.display_name.localeCompare(b.display_name)
      return a.name.localeCompare(b.name)
    })

    return sorted
  }, [assistants, searchTerm, selectedTags, sortBy])

  // Show loading state
  if (loading && assistants.length === 0) {
    return (
      <Loading tip="Loading assistants..." />
    )
  }

  // Show error state
  if (error && assistants.length === 0) {
    return (
      <ErrorState
        variant="page"
        resource="hub assistants"
        description="The hub catalog couldn't be loaded. Check your connection and try again."
        details={error}
        onRetry={() => Stores.HubAssistants.loadAssistants()}
        data-testid="hub-assistants-error"
      />
    )
  }

  return (
    <div className="assistants-hub-tab flex flex-col gap-3 h-full overflow-hidden pt-1">
      {/* Search and Filters */}
      <div className="px-3">
        <div className="flex gap-2 flex-wrap">
          <Input
            data-testid="hub-assistants-search-input"
            placeholder="Search assistants..."
            prefix={<Search />}
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
            aria-label="Search assistants"
          />

          <MultiSelect
            data-testid="hub-assistants-tags-multiselect"
            placeholder="Filter by tags"
            searchPlaceholder="Search tags..."
            emptyText="No tags found"
            value={selectedTags}
            onChange={(value: string[]) => setSelectedTags(value)}
            removeLabel={(label) => `Remove tag filter: ${label}`}
            className="flex-1"
            options={assistantTags.map(tag => ({
              value: tag,
              label: tag,
            }))}
            aria-label="Filter by tags"
          />

          <Combobox
            data-testid="hub-assistants-sort-combobox"
            placeholder="Sort by"
            searchPlaceholder="Sort by..."
            emptyText="No sort options"
            value={sortBy}
            onChange={(value: string) => setSortBy(value)}
            className="flex-1"
            options={[
              { value: 'name', label: 'ID' },
              { value: 'display_name', label: 'Display name' },
            ]}
            aria-label="Sort assistants"
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
              size="default"
              variant="ghost"
              icon={<Eraser />}
              onClick={clearAllFilters}
              aria-label="Clear all filters"
              data-testid="hub-assistants-clear-filters-btn"
            >
              Clear all
            </Button>
          </div>
        )}
      </div>

      {/* Assistants List — incompatible items hidden entirely. */}
      <div className="flex-1 overflow-auto p-3">
        {(() => {
          const indexById = new Map(
            (catalog?.items ?? [])
              .filter(it => it.category === 'assistant')
              .map(it => [it.name, it]),
          )
          const visible = filteredAssistants.filter(a => {
            const ix = indexById.get(a.name)
            return !ix || compatOf(ix, serverVersion).status === 'ok'
          })
          return (
            <>
              <div className="flex flex-col gap-3">
                {visible.map(assistant => (
                  <AssistantHubCard key={assistant.name} assistant={assistant} />
                ))}
              </div>
              {visible.length === 0 && (
                <div className="text-center py-12">
                  <Text type="secondary">
                    {assistants.length === 0
                      ? 'No assistants yet'
                      : 'No assistants match your search'}
                  </Text>
                </div>
              )}
            </>
          )
        })()}
      </div>

    </div>
  )
}
