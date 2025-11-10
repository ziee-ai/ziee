import { useState, useMemo } from 'react'
import { Input, Select, Typography, Spin, Button } from 'antd'
import { SearchOutlined, ClearOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { ModelHubCard } from './ModelHubCard'

const { Text } = Typography

export function ModelsHubTab() {
  const { models, loading, error } = Stores.HubModels // Auto-loads via __init__
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [sortBy, setSortBy] = useState('popular')

  const clearAllFilters = () => {
    setSearchTerm('')
    setSelectedTags([])
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
      filtered = filtered.filter(m =>
        m.name.toLowerCase().includes(search) ||
        m.display_name.toLowerCase().includes(search) ||
        m.description?.toLowerCase().includes(search)
      )
    }

    // Tags
    if (selectedTags.length > 0) {
      filtered = filtered.filter(m =>
        selectedTags.some(tag => m.tags?.includes(tag))
      )
    }

    // Sort
    filtered.sort((a, b) => {
      if (sortBy === 'popular') return (b.popularity_score || 0) - (a.popularity_score || 0)
      if (sortBy === 'name') return a.name.localeCompare(b.name)
      if (sortBy === 'size') return (a.size_gb || 0) - (b.size_gb || 0)
      return 0
    })

    return filtered
  }, [models, searchTerm, selectedTags, sortBy])

  // Show loading state
  if (loading && models.length === 0) {
    return (
      <div className="flex justify-center items-center h-full">
        <Spin size="large" />
        <Text className="ml-4">Loading models...</Text>
      </div>
    )
  }

  // Show error state
  if (error && models.length === 0) {
    return (
      <div className="text-center py-12">
        <Text type="danger">Failed to load models: {error}</Text>
        <div className="mt-4">
          <Button onClick={() => Stores.HubModels.loadModels()}>
            Retry
          </Button>
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
            placeholder="Search models..."
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
            options={modelTags.map(tag => ({
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
              { value: 'size', label: 'Size' },
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

      {/* Models List */}
      <div className="flex-1 overflow-auto px-3 pb-3">
        <div className="flex flex-col gap-3">
          {filteredModels.map(model => (
            <ModelHubCard key={model.id} model={model} />
          ))}
        </div>

        {filteredModels.length === 0 && (
          <div className="text-center py-12">
            <Text type="secondary">No models found</Text>
          </div>
        )}
      </div>
    </div>
  )
}
