import { useState, useMemo } from 'react'
import { Input, Select, Typography, Spin, Button } from 'antd'
import { SearchOutlined, ClearOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { AssistantHubCard } from './AssistantHubCard'
import { AssistantFormDrawer } from '@/modules/assistants/components/AssistantFormDrawer'

const { Text } = Typography

export function AssistantsHubTab() {
  const { assistants, loading, error } = Stores.HubAssistants // Auto-loads via __init__
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [sortBy, setSortBy] = useState('popular')

  const clearAllFilters = () => {
    setSearchTerm('')
    setSelectedTags([])
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
      filtered = filtered.filter(a =>
        a.name.toLowerCase().includes(search) ||
        a.display_name.toLowerCase().includes(search) ||
        a.description?.toLowerCase().includes(search)
      )
    }

    // Tags
    if (selectedTags.length > 0) {
      filtered = filtered.filter(a =>
        selectedTags.some(tag => a.tags?.includes(tag))
      )
    }

    // Sort (create a copy to avoid mutating read-only array from store)
    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === 'popular') return (b.popularity_score || 0) - (a.popularity_score || 0)
      if (sortBy === 'name') return a.name.localeCompare(b.name)
      return 0
    })

    return sorted
  }, [assistants, searchTerm, selectedTags, sortBy])

  // Show loading state
  if (loading && assistants.length === 0) {
    return (
      <div className="flex justify-center items-center h-full">
        <Spin size="large" />
        <Text className="ml-4">Loading assistants...</Text>
      </div>
    )
  }

  // Show error state
  if (error && assistants.length === 0) {
    return (
      <div className="text-center py-12">
        <Text type="danger">Failed to load assistants: {error}</Text>
        <div className="mt-4">
          <Button onClick={() => Stores.HubAssistants.loadAssistants()}>
            Retry
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="assistants-hub-tab flex flex-col gap-3 h-full overflow-hidden">
      {/* Search and Filters */}
      <div className="px-3">
        <div className="flex gap-2 flex-wrap">
          <Input
            placeholder="Search assistants..."
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
            options={assistantTags.map(tag => ({
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

      {/* Assistants List */}
      <div className="flex-1 overflow-auto px-3 pb-3">
        <div className="flex flex-col gap-3">
          {filteredAssistants.map(assistant => (
            <AssistantHubCard key={assistant.id} assistant={assistant} />
          ))}
        </div>

        {filteredAssistants.length === 0 && (
          <div className="text-center py-12">
            <Text type="secondary">No assistants found</Text>
          </div>
        )}
      </div>

      {/* Assistant Form Drawer */}
      <AssistantFormDrawer />
    </div>
  )
}
