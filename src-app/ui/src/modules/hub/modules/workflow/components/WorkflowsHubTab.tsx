import { Eraser, Search, Workflow } from 'lucide-react'
import { Input, MultiSelect, Button, Empty } from '@ziee/kit'
import { useMemo, useState } from 'react'
import { Loading } from '@/core/components/Loading'
import { compatOf } from '@/modules/hub/stores/hub-catalog-store'
import { WorkflowHubCard } from './WorkflowHubCard'
import { HubCatalog } from '@/modules/hub/stores/hub-catalog-store'
import { HubInstalled } from '@/modules/hub/stores/hub-installed-store'

export function WorkflowsHubTab() {
  const catalog = HubCatalog.catalog
  const loading = HubCatalog.loading
  const serverVersion = HubCatalog.serverVersion
  void HubInstalled.items
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])

  const items = useMemo(
    () => (catalog?.items ?? []).filter(it => it.category === 'workflow'),
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
    return <Loading tip="Loading workflows..." label="Loading workflows" />
  }

  return (
    <div className="flex flex-col gap-3 h-full overflow-hidden pt-1">
      <div className="px-3">
        <div className="flex gap-2 flex-wrap">
          <Input
            data-testid="hub-workflows-search-input"
            placeholder="Search workflows..."
            prefix={<Search />}
            value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
            aria-label="Search workflows"
          />
          <MultiSelect
            data-testid="hub-workflows-tags-multiselect"
            placeholder="Filter by tags"
            value={selectedTags}
            onChange={(values: string[]) => {
              setSelectedTags(values)
            }}
            className="flex-1"
            options={allTags.map(t => ({ value: t, label: t }))}
            aria-label="Filter workflows by tags"
            searchPlaceholder="Search tags..."
            emptyText="No tags found"
            removeLabel={(label) => `Remove ${label}`}
          />
        </div>
      </div>

      <div className="flex-1 overflow-auto p-3">
        <div className="flex flex-col gap-3">
          {filtered.map(item => (
            <WorkflowHubCard key={item.name} item={item} />
          ))}
        </div>
        {filtered.length === 0 &&
          (searchTerm || selectedTags.length > 0 ? (
            <Empty
              data-testid="hub-workflows-empty"
              icon={<Workflow />}
              title="No workflows match your search"
              description="Try a different search term or clear the active filters."
            >
              <Button
                variant="outline"
                icon={<Eraser />}
                onClick={() => {
                  setSearchTerm('')
                  setSelectedTags([])
                }}
                data-testid="hub-workflows-empty-clear-btn"
              >
                Clear filters
              </Button>
            </Empty>
          ) : (
            <Empty
              data-testid="hub-workflows-empty"
              icon={<Workflow />}
              title="No workflows in the hub yet"
              description="The hub catalog has no workflows to show right now — check back after a hub refresh."
            />
          ))}
      </div>
    </div>
  )
}
