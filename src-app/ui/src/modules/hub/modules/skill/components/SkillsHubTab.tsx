import { useMemo, useState } from 'react'
import { Text, MultiSelect, Input } from '@/components/ui'
import { SearchOutlined } from '@ant-design/icons'
import { Loading } from '@/core/components/Loading'
import { Stores } from '@/core/stores'
import { compatOf } from '@/modules/hub/stores/hub-catalog-store'
import { SkillHubCard } from './SkillHubCard'

export function SkillsHubTab() {
  // Subscribe to the shared catalog so the list re-renders on refresh.
  const catalog = Stores.HubCatalog.catalog
  const loading = Stores.HubCatalog.loading
  const serverVersion = Stores.HubCatalog.serverVersion
  // Touch the installed list so install badges stay fresh.
  void Stores.HubInstalled.items
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])

  const items = useMemo(
    () => (catalog?.items ?? []).filter(it => it.category === 'skill'),
    [catalog],
  )

  const allTags = useMemo(() => {
    const tags = new Set<string>()
    items.forEach(it => it.tags?.forEach(t => tags.add(t)))
    return Array.from(tags).sort()
  }, [items])

  const filtered = useMemo(() => {
    let result = items
    if (searchTerm) {
      const q = searchTerm.toLowerCase()
      result = result.filter(
        it =>
          it.name.toLowerCase().includes(q) ||
          (it.title ?? '').toLowerCase().includes(q) ||
          it.summary.toLowerCase().includes(q),
      )
    }
    if (selectedTags.length > 0) {
      result = result.filter(it =>
        selectedTags.some(t => (it.tags ?? []).includes(t)),
      )
    }
    return [...result]
      .filter(it => compatOf(it, serverVersion).status === 'ok')
      .sort((a, b) => a.name.localeCompare(b.name))
  }, [items, searchTerm, selectedTags, serverVersion])

  if (loading && items.length === 0) {
    return <Loading tip="Loading skills..." />
  }

  return (
    <div className="flex flex-col gap-3 h-full overflow-hidden">
      <div className="px-3">
        <div className="flex gap-2 flex-wrap">
          <Input
            placeholder="Search skills..."
            prefix={<SearchOutlined />}
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
            aria-label="Search skills"
          />
          <MultiSelect
            placeholder="Filter by tags"
            value={selectedTags}
            onChange={setSelectedTags}
            className="flex-1"
            searchPlaceholder="Search tags..."
            emptyText="No tags found"
            removeLabel={(label) => `Remove tag: ${label}`}
            options={allTags.map(t => ({ value: t, label: t }))}
            aria-label="Filter skills by tags"
          />
        </div>
      </div>

      <div className="flex-1 overflow-auto px-3 pb-3">
        <div className="flex flex-col gap-3">
          {filtered.map(item => (
            <SkillHubCard key={item.name} item={item} />
          ))}
        </div>
        {filtered.length === 0 && (
          <div className="text-center py-12">
            <Text type="secondary">
              {items.length === 0
                ? 'No skills in the hub yet'
                : 'No skills match your search'}
            </Text>
          </div>
        )}
      </div>
    </div>
  )
}
