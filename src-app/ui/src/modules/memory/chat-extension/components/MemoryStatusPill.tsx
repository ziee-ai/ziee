import { useEffect, useState } from 'react'
import { Tooltip, Dropdown } from '@/components/ui'
import { message } from '@/components/ui'
import { EyeOff, Lightbulb } from 'lucide-react'
import { Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'

type Mode = 'inherit' | 'on' | 'off'

/**
 * MemoryStatusPill — the per-conversation memory-mode pill rendered
 * in the chat composer's `toolbar_status` slot (next to the MCP /
 * assistant chips). Plan §7 Phase 5 "composer pill".
 *
 * Per-conversation memory_mode moved off the chat `conversations`
 * table into the memory-owned `conversation_memory_settings` table
 * (backend migration 76). Read/write via
 * `GET`/`PUT /api/conversations/{id}/memory-mode`, NOT inline on
 * the Conversation type (chat no longer knows memory's vocabulary).
 */
export function MemoryStatusPill() {
  // CRITICAL: read every Stores.X.field at the TOP, before any early
  // return. Each proxy access fires a useEffect; reading conditionally
  // after a guard triggers "Rendered more hooks than during the
  // previous render."
  const conversation = Stores.Chat.conversation
  const adminSettings = Stores.MemoryAdmin.settings
  const [mode, setMode] = useState<Mode>('inherit')
  const [loading, setLoading] = useState(false)

  // Fetch the current memory_mode for the active conversation. Soft-
  // fails to 'inherit' on any error — the pill stays interactive even
  // if the read raced a conversation switch.
  useEffect(() => {
    let cancelled = false
    if (!conversation?.id) {
      setMode('inherit')
      return
    }
    ;(async () => {
      try {
        const resp = await ApiClient.Conversation.getMemoryMode({
          id: conversation.id,
        })
        if (!cancelled) setMode((resp.memory_mode as Mode) ?? 'inherit')
      } catch {
        if (!cancelled) setMode('inherit')
      }
    })()
    return () => {
      cancelled = true
    }
  }, [conversation?.id])

  // Don't show the pill on the empty /chat landing, or when memory is
  // globally disabled by the admin (audit R6-#17 — pill is meaningless
  // when the deployment-wide setting is off).
  if (!conversation?.id) return null
  if (adminSettings?.enabled === false) return null

  async function setRemote(next: Mode) {
    if (!conversation?.id) return
    setLoading(true)
    try {
      await ApiClient.Conversation.setMemoryMode({
        id: conversation.id,
        memory_mode: next,
      })
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
      icon: <Lightbulb />,
    },
    { key: 'on', label: 'Always retrieve memories', icon: <Lightbulb /> },
    {
      key: 'off',
      label: "Don't use memories here",
      icon: <EyeOff />,
    },
  ]

  const labelByMode: Record<Mode, string> = {
    inherit: 'Memory: auto',
    on: 'Memory: on',
    off: 'Memory: off',
  }

  return (
    <Tooltip content="Per-conversation memory retrieval override">
      <Dropdown
        data-testid="memory-status-pill-dropdown"
        items={items}
        onSelect={(key) => setRemote(key as Mode)}
        disabled={loading}
      >
        <span
          data-testid="memory-status-pill"
          data-mode={mode}
          aria-label={`Memory mode: ${labelByMode[mode]}`}
          className="inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs font-medium"
          style={{ cursor: 'pointer' }}
        >
          {mode === 'off' ? <EyeOff /> : <Lightbulb />}
          {labelByMode[mode]}
        </span>
      </Dropdown>
    </Tooltip>
  )
}
