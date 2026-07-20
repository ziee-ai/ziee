import { useEffect, useState } from 'react'
import { Tooltip, Dropdown, Tag } from '@ziee/kit'
import { message } from '@ziee/kit'
import { EyeOff, Lightbulb } from 'lucide-react'
import { Stores } from '@ziee/framework/stores'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'

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
  // Permission gate (layer 4) — hooks first, early return AFTER every store read
  // (see the note below). Without `memory::read` the user has no Memory settings
  // page either, so an ungated pill would be a dead control.
  const canUse = usePermission({
    anyOf: [Permissions.MemoryRead, Permissions.CoreMemoryRead],
  })
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
  // Safe here: every hook + store read above has already run (see the CRITICAL
  // note at the top of this component).
  if (!canUse) return null
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
  // Mirror the Summary pill's tone mapping so the two composer-footer chips are
  // structurally identical peers (A9).
  const toneByMode: Record<Mode, Parameters<typeof Tag>[0]['tone']> = {
    inherit: undefined,
    on: 'success',
    off: 'error',
  }

  return (
    <Tooltip content="Per-conversation memory retrieval override">
      <Dropdown
        data-testid="memory-status-pill-dropdown"
        items={items}
        onSelect={(key) => setRemote(key as Mode)}
        disabled={loading}
      >
        {/* Route through the shared kit <Tag> (same as SummarizationStatusPill) so
            the icon size (Tag forces [&_svg]:size-3) + chip metrics are INHERITED,
            not per-set — the two footer chips were mismatched because this pill
            hand-rolled the markup and rendered a raw, unsized (24px) lucide icon. */}
        <Tag
          variant="outline"
          data-testid="memory-status-pill"
          data-mode={mode}
          tone={toneByMode[mode]}
          icon={mode === 'off' ? <EyeOff /> : <Lightbulb />}
          aria-label={`Memory mode: ${labelByMode[mode]}`}
          className="cursor-pointer m-0"
        >
          {labelByMode[mode]}
        </Tag>
      </Dropdown>
    </Tooltip>
  )
}
