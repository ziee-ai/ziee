import { Shrink, FileText, EyeOff, Loader2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Tooltip, Tag, Dropdown, message } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { ApiClient } from '@/api-client'
import { ConversationSummarization as ConversationSummarizationStore } from '@/modules/summarization/stores/conversationSummarization'
import { SummarizationAdmin as SummarizationAdminStore } from '@/modules/summarization/stores/summarizationAdmin'

type Mode = 'inherit' | 'on' | 'off'

/**
 * SummarizationStatusPill — per-conversation summarization-mode pill
 * in the chat composer's `toolbar_status` slot. Mirrors
 * `MemoryStatusPill` (memory's per-conversation pill).
 *
 * Also acts as the **read-model driver** for the in-thread summary
 * marker: subscribes to `messages.size` + `conversation.id` and calls
 * `ConversationSummarizationStore.loadForConversation(id)`
 * on change. This load-bearing pattern rides cross-device freshness
 * transitively on `sync:conversation` — DO NOT move the trigger
 * elsewhere (audit lesson from the crashed-session redo).
 */
export function SummarizationStatusPill() {
  // Read every Stores.X.field at the TOP, before any conditional.
  // Each proxy access fires a useEffect; reading conditionally after
  // a guard triggers "Rendered more hooks than during the previous
  // render."
  const conversation = Stores.Chat.conversation
  const messages = Stores.Chat.messages
  const adminSettings = SummarizationAdminStore.settings
  const [mode, setMode] = useState<Mode>('inherit')
  const [loading, setLoading] = useState(false)

  // Per-conversation mode fetch. Soft-fails to 'inherit' on any error
  // (the pill stays interactive even if the read raced a switch).
  useEffect(() => {
    let cancelled = false
    if (!conversation?.id) {
      setMode('inherit')
      return
    }
    ;(async () => {
      try {
        const resp = await ApiClient.Conversation.getSummarizationMode({
          id: conversation.id,
        })
        if (!cancelled)
          setMode((resp.summarization_mode as Mode) ?? 'inherit')
      } catch {
        if (!cancelled) setMode('inherit')
      }
    })()
    return () => {
      cancelled = true
    }
  }, [conversation?.id])

  // Drive the summary read-model: re-fetch when the conversation
  // changes OR when message count changes (a new turn just landed,
  // and the summary might have been updated by the after_llm_call
  // hook on the server). The single-entry cache in
  // ConversationSummarization rotates on conversation switch.
  useEffect(() => {
    if (!conversation?.id) {
      ConversationSummarizationStore.clear()
      return
    }
    void ConversationSummarizationStore.loadForConversation(
      conversation.id,
    )
  }, [conversation?.id, messages.size])

  if (!conversation?.id) return null
  // Known cross-cutting limitation (mirrors MemoryStatusPill): for
  // non-admins, `adminSettings` stays null because
  // `SummarizationAdmin.__init__.settings` is self-gated on
  // `summarization::settings::read`. So `null?.enabled === false` is
  // false, and the pill shows for non-admins even when the admin
  // disabled summarization deployment-wide. The deeper fix is a
  // public-readable `enabled` flag served alongside `/auth/me`;
  // tracked with memory as a single follow-up so the two pills don't
  // drift apart in the meantime.
  if (adminSettings?.enabled === false) return null

  // Per-conversation mode is fetched on `conversation.id` change above.
  // It deliberately has NO `sync:conversation` subscription: toggling
  // mode on device A is not visible on device B until next conv switch
  // or `messages.size` change. This matches the backend (the PUT
  // handler emits no Conversation sync event — same shape as memory's
  // memory-mode endpoint).

  async function setRemote(next: Mode) {
    if (!conversation?.id) return
    setLoading(true)
    try {
      await ApiClient.Conversation.setSummarizationMode({
        id: conversation.id,
        summarization_mode: next,
      })
      setMode(next)
      message.success(`Summarization: ${next} for this conversation`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to update summarization mode')
    } finally {
      setLoading(false)
    }
  }

  const items = [
    {
      key: 'inherit',
      label: 'Inherit (follow deployment setting)',
      icon: <Shrink />,
    },
    { key: 'on', label: 'Always summarize this conversation', icon: <FileText /> },
    {
      key: 'off',
      label: 'Never summarize this conversation',
      icon: <EyeOff />,
    },
  ]

  const labelByMode: Record<Mode, string> = {
    inherit: 'Summary: auto',
    on: 'Summary: on',
    off: 'Summary: off',
  }
  const toneByMode: Record<Mode, Parameters<typeof Tag>[0]['tone']> = {
    inherit: undefined,
    on: 'success',
    off: 'error',
  }

  return (
    <Tooltip content="Per-conversation summarization override">
      <Dropdown
        data-testid="summ-mode-dropdown"
        items={items}
        onSelect={(key) => setRemote(key as Mode)}
        disabled={loading}
        nativeButton={false}
      >
        <Tag variant="outline"
          data-testid="summ-mode-tag"
          tone={toneByMode[mode]}
          icon={
            loading ? (
              <Loader2 className="animate-spin" />
            ) : mode === 'off' ? (
              <EyeOff />
            ) : (
              <Shrink />
            )
          }
          aria-label={`Summarization override: ${labelByMode[mode]}`}
          className="cursor-pointer m-0"
        >
          {labelByMode[mode]}
        </Tag>
      </Dropdown>
    </Tooltip>
  )
}
