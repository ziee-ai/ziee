import { useEffect, useState } from 'react'
import { Tooltip, Tag, Dropdown, message } from 'antd'
import { BulbOutlined, BulbFilled, EyeInvisibleOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

type Mode = 'inherit' | 'on' | 'off'

/**
 * MemoryStatusPill — the per-conversation memory-mode pill rendered
 * in the chat composer's `toolbar_status` slot (next to the MCP /
 * assistant chips). Plan §7 Phase 5 "composer pill".
 *
 * Reads the active `Stores.Chat.conversation.memory_mode` and PATCHes
 * /api/conversations/{id} to change it. Hidden until a conversation
 * is loaded (initial /chat page with no selection shows nothing).
 */
export function MemoryStatusPill() {
  const conversation = Stores.Chat.conversation
  const [mode, setMode] = useState<Mode>('inherit')
  const [loading, setLoading] = useState(false)

  // Re-sync the local mode state whenever the active conversation
  // changes. `memory_mode` was added to Conversation via OpenAPI but
  // the cast guards against stale api-client types.
  const conversationMemoryMode = (
    conversation as unknown as { memory_mode?: Mode }
  )?.memory_mode
  useEffect(() => {
    setMode(conversationMemoryMode ?? 'inherit')
  }, [conversation?.id, conversationMemoryMode])

  // Don't show the pill on the empty /chat landing, or when memory is
  // globally disabled by the admin (audit R6-#17 — pill is meaningless
  // when the deployment-wide setting is off).
  if (!conversation?.id) return null
  const adminEnabled = Stores.MemoryAdmin?.settings?.enabled
  if (adminEnabled === false) return null

  async function setRemote(next: Mode) {
    if (!conversation?.id) return
    setLoading(true)
    try {
      const res = await fetch(`/api/conversations/${conversation.id}`, {
        method: 'PUT',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ memory_mode: next }),
      })
      if (!res.ok) throw new Error(`Failed: ${res.status}`)
      setMode(next)
      message.success(`Memory: ${next} for this conversation`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to update memory mode')
    } finally {
      setLoading(false)
    }
  }

  const items = [
    {
      key: 'inherit',
      label: 'Inherit (follow account setting)',
      icon: <BulbOutlined />,
    },
    { key: 'on', label: 'Always retrieve memories', icon: <BulbFilled /> },
    {
      key: 'off',
      label: "Don't use memories here",
      icon: <EyeInvisibleOutlined />,
    },
  ]

  const labelByMode: Record<Mode, string> = {
    inherit: 'Memory: auto',
    on: 'Memory: on',
    off: 'Memory: off',
  }
  const colorByMode: Record<Mode, string> = {
    inherit: 'default',
    on: 'green',
    off: 'red',
  }

  return (
    <Tooltip title="Per-conversation memory retrieval override">
      <Dropdown
        menu={{
          items,
          selectable: true,
          selectedKeys: [mode],
          onClick: ({ key }) => setRemote(key as Mode),
        }}
        disabled={loading}
      >
        <Tag
          color={colorByMode[mode]}
          icon={mode === 'off' ? <EyeInvisibleOutlined /> : <BulbOutlined />}
          style={{ cursor: 'pointer', margin: 0 }}
        >
          {labelByMode[mode]}
        </Tag>
      </Dropdown>
    </Tooltip>
  )
}
