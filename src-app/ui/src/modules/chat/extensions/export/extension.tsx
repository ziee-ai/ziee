import { Button, Dropdown, message } from 'antd'
import { DownloadOutlined } from '@ant-design/icons'
import type { MenuProps } from 'antd'
import {
  createExtension,
  type ChatExtension,
} from '../../core/extensions'
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
function getExportMenuItems(): MenuProps['items'] {
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
 * Export button component
 */
function ExportButton() {
  const messages = Array.from(Stores.Chat.messages.values())

  // Don't show export button if no messages
  if (messages.length === 0) {
    return null
  }

  return (
    <Dropdown menu={{ items: getExportMenuItems() }} placement="bottomRight">
      <Button icon={<DownloadOutlined />} size="small">
        Export
      </Button>
    </Dropdown>
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

  // Register slot components
  slots: {
    toolbar_actions: { component: ExportButton, order: 70 },
  },
})

export default exportExtension
