import { useEffect, useMemo, useRef, useState } from 'react'
import { Button, Input, Text } from '@/components/ui'
import { ChevronDown, ChevronUp, X } from 'lucide-react'
import { Stores } from '@/core/stores'
import { findMatches } from '@/modules/chat/components/findMatches'

interface ConversationFindBarProps {
  open: boolean
  onClose: () => void
  /** Report the id of the active match (or null) so the page can highlight it. */
  onActiveMatchChange: (id: string | null) => void
}

/**
 * ConversationFindBar (ITEM-1) — find-within-open-conversation.
 *
 * Searches the already-loaded messages (`Stores.Chat.messages`) for a term,
 * shows an "X of Y" readout, and Next/Prev (or Enter / Shift+Enter) scroll each
 * matched message into view. The active match id is reported upward so the
 * matched message gets a highlight ring (via ConversationFindContext). Esc or
 * the close button dismisses and clears the highlight.
 */
export function ConversationFindBar({
  open,
  onClose,
  onActiveMatchChange,
}: ConversationFindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null)
  const [query, setQuery] = useState('')
  const [index, setIndex] = useState(0)

  const { messages } = Stores.Chat
  const messagesArray = useMemo(() => Array.from(messages.values()), [messages])

  const matches = useMemo(
    () => findMatches(messagesArray, query),
    [messagesArray, query],
  )
  const matchesKey = matches.join(',')

  // Focus the input whenever the bar opens.
  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  // Reset to the first match whenever the match set changes (new query / edited
  // conversation). Clamp so a shrinking match set never leaves a stale index.
  useEffect(() => {
    setIndex(0)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [matchesKey])

  // Scroll the active match into view + report it upward for highlighting.
  useEffect(() => {
    if (!open) return
    const id = matches[index] ?? null
    onActiveMatchChange(id)
    if (id) {
      document
        .querySelector(`[data-message-id="${CSS.escape(id)}"]`)
        ?.scrollIntoView({ behavior: 'smooth', block: 'center' })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, index, matchesKey])

  // Clear the highlight when the bar closes.
  useEffect(() => {
    if (!open) onActiveMatchChange(null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  if (!open) return null

  const total = matches.length
  const go = (delta: number) => {
    if (total === 0) return
    setIndex(i => (i + delta + total) % total)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      go(e.shiftKey ? -1 : 1)
    } else if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
  }

  return (
    <div
      className="flex items-center gap-2 rounded-lg border border-border bg-card p-1.5 shadow-md"
      data-testid="conversation-find-bar"
      role="search"
    >
      <Input
        ref={inputRef}
        data-testid="conversation-find-input"
        aria-label="Find in conversation"
        placeholder="Find in conversation..."
        value={query}
        onChange={e => setQuery(e.target.value)}
        onKeyDown={handleKeyDown}
        className="h-8 w-48"
      />
      <Text
        type="secondary"
        className="min-w-14 text-center text-xs tabular-nums"
        aria-live="polite"
        data-testid="conversation-find-count"
      >
        {query.trim() === ''
          ? ' '
          : total === 0
            ? 'No results'
            : `${index + 1} of ${total}`}
      </Text>
      <Button
        data-testid="conversation-find-prev"
        variant="ghost"
        size="icon"
        icon={<ChevronUp />}
        aria-label="Previous match"
        disabled={total === 0}
        onClick={() => go(-1)}
      />
      <Button
        data-testid="conversation-find-next"
        variant="ghost"
        size="icon"
        icon={<ChevronDown />}
        aria-label="Next match"
        disabled={total === 0}
        onClick={() => go(1)}
      />
      <Button
        data-testid="conversation-find-close"
        variant="ghost"
        size="icon"
        icon={<X />}
        aria-label="Close find"
        onClick={onClose}
      />
    </div>
  )
}
