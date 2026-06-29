import { Eraser, Search } from 'lucide-react'
import { useState, useMemo } from 'react'
import { MultiSelect, Select, Text, Button, Input } from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { Stores } from '@/core/stores'
import { ModelHubCard } from '@/modules/hub/modules/llm-models/components/ModelHubCard'
import { compatOf } from '@/modules/hub/stores/hub-catalog-store'

export function ModelsHubTab() {
  const { models, loading, error } = Stores.HubModels // Auto-loads via __init__
  // Cross-reference each model id with the catalog so we know its
  // min_ziee_version. The catalog store loads /hub/index lazily.
  const catalog = Stores.HubCatalog.catalog
  const serverVersion = Stores.HubCatalog.serverVersion
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  // v2 Phase 7 dropped `popularity_score` + the model-wide `size_gb`,
  // so the sort options are reduced to name. A future revision could
  // sort by the default quantization's `sizeGb` across all sources.
  const [sortBy, setSortBy] = useState('name')

  const clearAllFilters = () => {
    setSearchTerm('')
    setSelectedTags([])
    setSortBy('name')
  }

  // Extract unique tags
  const modelTags = useMemo(() => {
    const allTags = new Set<string>()
    models.forEach(model => {
      model.tags?.forEach(tag => allTags.add(tag))
    })
    return Array.from(allTags).sort()
  }, [models])

  // Filter and sort
  const filteredModels = useMemo(() => {
    let filtered = models

    // Search
    if (searchTerm) {
      const search = searchTerm.toLowerCase()
      filtered = filtered.filter(
        m =>
          m.name.toLowerCase().includes(search) ||
          m.display_name.toLowerCase().includes(search) ||
          m.description?.toLowerCase().includes(search),
      )
    }

    // Tags
    if (selectedTags.length > 0) {
      filtered = filtered.filter(m =>
        selectedTags.some(tag => m.tags?.includes(tag)),
      )
    }

    // Sort (create a copy to avoid mutating read-only array from store).
    // v2 Phase 7: `popularity_score` + `size_gb` are gone; sort by
    // name (canonical reverse-DNS) or display_name only.
    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === 'display_name')
        return a.display_name.localeCompare(b.display_name)
      // fall-through: 'name'
      return a.name.localeCompare(b.name)
    })

    return sorted
  }, [models, searchTerm, selectedTags, sortBy])

  // Show loading state
  if (loading && models.length === 0) {
    return (
      <Loading tip="Loading models..." />
    )
  }

  // Show error state
  if (error && models.length === 0) {
    return (
      <div className="text-center py-12">
        <Text type="danger">Failed to load models: {error}</Text>
        <div className="mt-4">
          <Button onClick={() => Stores.HubModels.loadModels()} data-testid="hub-models-retry-btn">Retry</Button>
        </div>
      </div>
    )
  }

  return (
    <div className="models-hub-tab flex flex-col gap-3 h-full overflow-hidden">
      {/* Search and Filters */}
      <div className="px-3">
        <div className="flex gap-2 flex-wrap">
          <Input
            data-testid="hub-models-search-input"
            placeholder="Search models..."
            prefix={<Search />}
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
            aria-label="Search models"
          />

          <MultiSelect
            data-testid="hub-models-tags-multiselect"
            placeholder="Filter by tags"
            value={selectedTags}
            onChange={(values: string[]) => setSelectedTags(values)}
            className="flex-1"
            removeLabel={(label) => `Remove ${label}`}
            emptyText="No tags available"
            searchPlaceholder="Search tags..."
            options={modelTags.map(tag => ({
              value: tag,
              label: tag,
            }))}
            aria-label="Filter by tags"
          />

          <Select
            data-testid="hub-models-sort-select"
            placeholder="Sort by"
            value={sortBy}
            onChange={(value: string) => setSortBy(value)}
            className="flex-1"
            options={[
              { value: 'name', label: 'ID' },
              { value: 'display_name', label: 'Display name' },
            ]}
            aria-label="Sort models"
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
              size="sm"
              variant="ghost"
              icon={<Eraser />}
              onClick={clearAllFilters}
              aria-label="Clear all filters"
              data-testid="hub-models-clear-filters-btn"
            >
              Clear all
            </Button>
          </div>
        )}
      </div>

      {/* Models List — incompatible items (min_ziee_version > server)
          are hidden entirely; the backend also rejects installing them. */}
      <div className="flex-1 overflow-auto px-3 pb-3">
        {(() => {
          const indexById = new Map(
            (catalog?.items ?? [])
              .filter(it => it.category === 'model')
              .map(it => [it.name, it]),
          )
          // Show items that are compatible OR not in the catalog index
          // (orphans / dev models are never hidden).
          const visibleModels = filteredModels.filter(m => {
            const ix = indexById.get(m.name)
            return !ix || compatOf(ix, serverVersion).status === 'ok'
          })
          return (
            <>
              <div className="flex flex-col gap-3">
                {visibleModels.map(model => (
                  <ModelHubCard key={model.name} model={model} />
                ))}
              </div>
              {visibleModels.length === 0 && (
                <div className="text-center py-12" data-testid="hub-models-empty">
                  <Text type="secondary">
                    {models.length === 0
                      ? 'No models yet'
                      : 'No models match your search'}
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
