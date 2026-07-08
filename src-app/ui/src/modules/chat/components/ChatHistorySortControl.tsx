import { ArrowUpDown, Check } from 'lucide-react'
import { Button, Dropdown, Select } from '@/components/ui'
import type { DropdownItem } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { ConversationSort } from '@/modules/chat/stores/ChatHistory.store'

export const SORT_OPTIONS: { value: ConversationSort; label: string }[] = [
  { value: 'recent', label: 'Most recent' },
  { value: 'oldest', label: 'Oldest first' },
  { value: 'alpha', label: 'Title A–Z' },
  { value: 'most_messages', label: 'Most messages' },
]

/**
 * Chat-history sort control, rendered in the page header (between the search and
 * the new-chat button). Wide: a labelled Select. Narrow (`iconOnly`, the same
 * breakpoint that collapses the search into an icon): an icon-only button with a
 * dropdown, the active option check-marked.
 */
export function ChatHistorySortControl({ iconOnly }: { iconOnly?: boolean }) {
  const { sort } = Stores.ChatHistory

  if (iconOnly) {
    const items: DropdownItem[] = SORT_OPTIONS.map(o => ({
      key: o.value,
      label: o.label,
      icon: sort === o.value ? <Check /> : <span className="size-4" />,
      onClick: () => Stores.ChatHistory.setSort(o.value),
    }))
    return (
      <Dropdown items={items} data-testid="chat-history-sort-dropdown">
        <Button
          data-testid="chat-history-sort-btn"
          variant="ghost"
          icon={<ArrowUpDown />}
          aria-label="Sort conversations"
          tooltip="Sort"
        />
      </Dropdown>
    )
  }

  return (
    <Select
      data-testid="chat-history-sort-select"
      aria-label="Sort conversations"
      value={sort}
      onChange={value => Stores.ChatHistory.setSort(value as ConversationSort)}
      options={SORT_OPTIONS}
      className="w-40"
    />
  )
}
