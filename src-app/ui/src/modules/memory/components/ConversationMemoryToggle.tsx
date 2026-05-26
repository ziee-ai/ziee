import { useEffect, useState } from 'react'
import { Tooltip, Tag, Dropdown, message } from 'antd'
import { BulbOutlined, BulbFilled, EyeInvisibleOutlined } from '@ant-design/icons'

type Mode = 'inherit' | 'on' | 'off'

/**
 * Per-conversation memory toggle pill. Plan §7 Phase 5.
 *
 * Drop this into the chat composer (or anywhere convenient) to give
 * the user a one-click override of `conversations.memory_mode`:
 *   - `inherit` (default) — follow the user-level retrieval_enabled.
 *   - `on` — force retrieval for this conversation regardless.
 *   - `off` — suppress retrieval for this conversation regardless.
 *
 * Extraction is not gated per-conversation (plan keeps that
 * user-level only); this pill is RETRIEVAL only.
 */
export function ConversationMemoryToggle({ conversationId }: { conversationId: string }) {
  const [mode, setMode] = useState<Mode>('inherit')
  const [loading, setLoading] = useState(false)

  // Load current mode.
  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const res = await fetch(`/api/conversations/${conversationId}`, {
          credentials: 'include',
        })
        if (res.ok) {
          const c = await res.json()
          if (!cancelled && c.memory_mode) {
            setMode(c.memory_mode as Mode)
          }
        }
      } catch {
        /* silent — pill defaults to 'inherit' if fetch fails */
      }
    })()
    return () => {
      cancelled = true
    }
  }, [conversationId])

  async function setRemote(next: Mode) {
    setLoading(true)
    try {
      const res = await fetch(`/api/conversations/${conversationId}`, {
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
          style={{ cursor: 'pointer' }}
        >
          {labelByMode[mode]}
        </Tag>
      </Dropdown>
    </Tooltip>
  )
}
