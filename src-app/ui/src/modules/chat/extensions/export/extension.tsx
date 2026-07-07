import { Popover, message } from '@/components/ui'
import { Download, ChevronRight } from 'lucide-react'
import {
  createExtension,
  type ChatExtension,
} from '@/modules/chat/core/extensions'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { PlusMenuItem } from '@/modules/chat/components/PlusMenuItem'
import type { MessageWithContent } from '@/api-client/types'
import { Stores } from '@/core/stores'

/**
 * Extract text from message contents
 */
function extractMessageText(message: MessageWithContent): string {
  if (!message.contents || message.contents.length === 0) {
    return ''
  }

  return message.contents
    .filter(content => content.content_type === 'text')
    .map(content => {
      const textData = content.content as { text?: string }
      return textData.text || ''
    })
    .join('\n')
}

/**
 * Export conversation as JSON
 */
function exportAsJSON(): void {
  // Access raw state outside React context
  const { conversation, messages } = Stores.Chat.__state
  if (!conversation) return

  const messagesArray = Array.from(messages.values())

  const data = {
    conversationId: conversation.id,
    branchId: conversation.active_branch_id || '',
    exportedAt: new Date().toISOString(),
    messages: messagesArray.map(msg => ({
      id: msg.id,
      role: msg.role,
      text: extractMessageText(msg),
      created_at: msg.created_at,
    })),
  }

  const blob = new Blob([JSON.stringify(data, null, 2)], {
    type: 'application/json',
  })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `conversation-${conversation.id.slice(0, 8)}.json`
  a.click()
  URL.revokeObjectURL(url)

  message.success('Conversation exported as JSON')
}

/**
 * Export conversation as plain text
 */
function exportAsText(): void {
  // Access raw state outside React context
  const { conversation, messages } = Stores.Chat.__state
  if (!conversation) return

  const messagesArray = Array.from(messages.values())

  const text = messagesArray
    .map(msg => {
      const role = msg.role === 'user' ? 'User' : 'Assistant'
      const content = extractMessageText(msg)
      return `${role}:\n${content}\n`
    })
    .join('\n---\n\n')

  const blob = new Blob([text], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `conversation-${conversation.id.slice(0, 8)}.txt`
  a.click()
  URL.revokeObjectURL(url)

  message.success('Conversation exported as text')
}

/**
 * Export conversation as Markdown
 */
function exportAsMarkdown(): void {
  // Access raw state outside React context
  const { conversation, messages } = Stores.Chat.__state
  if (!conversation) return

  const messagesArray = Array.from(messages.values())

  const markdown = messagesArray
    .map(msg => {
      const role = msg.role === 'user' ? '**User**' : '**Assistant**'
      const content = extractMessageText(msg)
      return `${role}:\n\n${content}\n`
    })
    .join('\n---\n\n')

  const blob = new Blob([markdown], { type: 'text/markdown' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `conversation-${conversation.id.slice(0, 8)}.md`
  a.click()
  URL.revokeObjectURL(url)

  message.success('Conversation exported as Markdown')
}

/**
 * Export menu items
 */
function getExportMenuItems() {
  return [
    {
      key: 'json',
      label: 'Export as JSON',
      onClick: () => exportAsJSON(),
    },
    {
      key: 'txt',
      label: 'Export as Text',
      onClick: () => exportAsText(),
    },
    {
      key: 'md',
      label: 'Export as Markdown',
      onClick: () => exportAsMarkdown(),
    },
  ]
}

/**
 * Export entry INSIDE the "+" dropdown. Renders a row visually identical to its
 * sibling "+" menu items (Attach files / Skills / MCP / Assistant): same
 * PlusMenuItem wrapper + a trailing chevron. The export FORMAT choice
 * (JSON / Text / Markdown) opens to the RIGHT as a nested Popover — the SAME
 * mechanism the "Select assistant" item uses. A Popover (z-[70]) is used rather
 * than a Dropdown (z-[60]) so the format panel layers ABOVE the "+" Popover
 * instead of behind it; picking a format exports and closes the "+" menu.
 */
function ExportMenuItem() {
  const messages = Array.from(Stores.Chat.messages.values())
  const { close } = usePlusDropdown()

  // Don't show export if there's nothing to export.
  if (messages.length === 0) {
    return null
  }

  const formatMenu = (
    <div className="flex flex-col">
      {getExportMenuItems().map(it => (
        <div
          key={it.key}
          role="button"
          tabIndex={0}
          data-testid={`chat-export-format-${it.key}`}
          className="cursor-pointer rounded-md px-3 py-1.5 text-sm text-foreground whitespace-nowrap hover:bg-muted focus-visible:outline focus-visible:outline-2"
          onClick={() => {
            it.onClick()
            close()
          }}
          onKeyDown={e => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              it.onClick()
              close()
            }
          }}
        >
          {it.label}
        </div>
      ))}
    </div>
  )

  return (
    <Popover content={formatMenu} side="right" align="start" className="w-auto">
      {/* Trailing chevron marks this as opening a submenu — same affordance +
          mechanism as the "Select assistant" item. */}
      <PlusMenuItem
        data-testid="chat-export-menu-item"
        aria-label="Export conversation"
        icon={<Download />}
        label="Export conversation"
        trailing={<ChevronRight className="size-3 opacity-45" />}
      />
    </Popover>
  )
}

/**
 * Export Extension
 * Provides conversation export functionality (JSON, text, markdown)
 */
const exportExtension: ChatExtension = createExtension({
  name: 'export',
  description: 'Conversation export functionality',
  priority: 70,

  // No store needed - stateless extension

  // Register slot components: Export lives INSIDE the "+" dropdown (a peer of
  // Attach files / Skills / MCP / Assistant), not as a standalone toolbar button.
  slots: {
    toolbar_plus_items: { component: ExportMenuItem, order: 70 },
  },
})

export default exportExtension
