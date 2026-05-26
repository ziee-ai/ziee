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
  // changes (user navigated to a different one, branch switch, etc.).
  useEffect(() => {
    const current = (conversation as any)?.memory_mode as Mode | undefined
    if (current) {
      setMode(current)
    } else {
      setMode('inherit')
    }
  }, [conversation?.id, (conversation as any)?.memory_mode])

  // Don't show the pill on the empty /chat landing.
  if (!conversation?.id) return null

  async function setRemote(next: Mode) {
    if (!conversation?.id) return
    setLoading(true)
    try {
      const res = await fetch(`/api/conversations/${conversation.id}`, {
        method: 'PATCH',
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
