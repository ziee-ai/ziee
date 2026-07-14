import { useCallback, useEffect, useRef, useState } from 'react'
import { Button, Input, Text } from '@ziee/kit'
import { ChevronDown, ChevronUp, Loader2, X } from 'lucide-react'
import { Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type { MessageSearchMatch } from '@/api-client/types'
import { cn } from '@/lib/utils'

interface ConversationFindBarProps {
  open: boolean
  onClose: () => void
  /** Report the id of the active match (or null) so the page can highlight it. */
  onActiveMatchChange: (id: string | null) => void
  /** Scroll a (loaded) message into view via the virtualizer. Returns false when
   *  it isn't in the loaded window (the caller has already jumped by then). */
  scrollToMessage: (id: string) => boolean
}

const SEARCH_DEBOUNCE_MS = 250
const SEARCH_PER_PAGE = 25

/**
 * ConversationFindBar (ITEM-13) — find-within-conversation, SERVER-SIDE.
 *
 * Under lazy-load the client only holds a WINDOW of the conversation, so find
 * runs against the backend (`Message.searchInConversation`) over the whole
 * active branch — matches in not-yet-loaded messages are still found. Results
 * are displayed as a paginated snippet LIST (infinite-scrolls to the next page);
 * "X of Y" + Next/Prev step across the full match set. Selecting/navigating a
 * match scrolls it into view when it's loaded, else jumps to it via `around=`
 * (`Stores.Chat.jumpToMessage`) and then centers + highlights it (via the
 * `onActiveMatchChange` ring). Esc / close dismisses and clears the highlight.
 */
export function ConversationFindBar({
  open,
  onClose,
  onActiveMatchChange,
  scrollToMessage,
}: ConversationFindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null)
  const resultsRef = useRef<HTMLDivElement>(null)
  // Monotonic search generation — bumped whenever the query/conversation
  // changes so a late in-flight page fetch (first page OR a paginated append)
  // can detect it's stale and discard its result instead of corrupting the
  // current match set / total.
  const searchGenRef = useRef(0)
  const [query, setQuery] = useState('')
  const [matches, setMatches] = useState<MessageSearchMatch[]>([])
  const [total, setTotal] = useState(0)
  const [loadedPage, setLoadedPage] = useState(0)
  const [activeIndex, setActiveIndex] = useState(0)
  const [loading, setLoading] = useState(false)

  const { conversation } = Stores.Chat
  const conversationId = conversation?.id
  const canLoadMore = matches.length < total

  // Focus the input whenever the bar opens.
  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  // Scroll a match into view + highlight it, jumping (around=) first when the
  // message isn't currently in the loaded window.
  const activateMatch = useCallback(
    async (match: MessageSearchMatch | undefined) => {
      if (!match) return
      const id = match.message_id
      onActiveMatchChange(id)
      if (!Stores.Chat.$.messages.has(id)) {
        const ok = await Stores.Chat.jumpToMessage(id)
        if (!ok) return
      }
      // Allow the (possibly newly-jumped) window to render, then scroll the
      // (possibly virtualized-out) match into view via the virtualizer.
      requestAnimationFrame(() => {
        scrollToMessage(id)
      })
    },
    [onActiveMatchChange, scrollToMessage],
  )

  // Debounced first-page search whenever the query (or conversation) changes.
  useEffect(() => {
    if (!open) return
    // Invalidate any in-flight fetch from a prior query/conversation.
    searchGenRef.current += 1
    const gen = searchGenRef.current
    const term = query.trim()
    if (!conversationId || term === '') {
      setMatches([])
      setTotal(0)
      setLoadedPage(0)
      setActiveIndex(0)
      onActiveMatchChange(null)
      return
    }
    let cancelled = false
    const timer = setTimeout(async () => {
      setLoading(true)
      try {
        const res = await ApiClient.Message.searchInConversation({
          id: conversationId,
          q: term,
          page: 1,
          per_page: SEARCH_PER_PAGE,
        })
        if (cancelled || gen !== searchGenRef.current) return
        setMatches(res.matches)
        setTotal(res.total)
        setLoadedPage(1)
        setActiveIndex(0)
        void activateMatch(res.matches[0])
      } catch {
        if (!cancelled) {
          setMatches([])
          setTotal(0)
          setLoadedPage(0)
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }, SEARCH_DEBOUNCE_MS)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
    // onActiveMatchChange / activateMatch are stable enough; re-running on every
    // identity change would refire the search needlessly.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, conversationId, open])

  // Clear the highlight when the bar closes.
  useEffect(() => {
    if (!open) onActiveMatchChange(null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  // Keep the ACTIVE result row visible inside the scrollable results list as
  // Next/Prev (or a jump beyond the loaded page) moves the selection — without
  // this the active row can be off-screen within the max-height list.
  useEffect(() => {
    if (!open) return
    resultsRef.current
      ?.querySelector('[aria-current="true"]')
      ?.scrollIntoView({ block: 'nearest' })
  }, [activeIndex, matches, open])

  // Append the next results page. Returns the newly-appended matches.
  const loadNextPage = useCallback(async (): Promise<MessageSearchMatch[]> => {
    if (!conversationId || loading) return []
    const term = query.trim()
    if (term === '') return []
    const nextPage = loadedPage + 1
    const gen = searchGenRef.current
    setLoading(true)
    try {
      const res = await ApiClient.Message.searchInConversation({
        id: conversationId,
        q: term,
        page: nextPage,
        per_page: SEARCH_PER_PAGE,
      })
      // Discard if the query/conversation changed while this page was in flight
      // — otherwise old-query matches would append onto the new list.
      if (gen !== searchGenRef.current) return []
      setMatches(prev => [...prev, ...res.matches])
      setTotal(res.total)
      setLoadedPage(nextPage)
      return res.matches
    } catch {
      return []
    } finally {
      if (gen === searchGenRef.current) setLoading(false)
    }
  }, [conversationId, loadedPage, loading, query])

  if (!open) return null

  const go = async (delta: number) => {
    if (total === 0) return
    if (delta > 0) {
      if (activeIndex < matches.length - 1) {
        const next = activeIndex + 1
        setActiveIndex(next)
        void activateMatch(matches[next])
      } else if (canLoadMore) {
        const prevLen = matches.length
        const more = await loadNextPage()
        if (more.length > 0) {
          setActiveIndex(prevLen)
          void activateMatch(more[0])
        }
      }
    } else if (activeIndex > 0) {
      const prev = activeIndex - 1
      setActiveIndex(prev)
      void activateMatch(matches[prev])
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      void go(e.shiftKey ? -1 : 1)
    } else if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
  }

  const handleResultsScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget
    if (
      canLoadMore &&
      !loading &&
      el.scrollHeight - el.scrollTop - el.clientHeight < 48
    ) {
      void loadNextPage()
    }
  }

  const countLabel =
    query.trim() === ''
      ? ' '
      : loading && total === 0
        ? 'Searching…'
        : total === 0
          ? 'No results'
          : `${activeIndex + 1} of ${total}`

  return (
    <div
      className="flex w-72 flex-col gap-1.5 rounded-lg border border-border bg-card p-1.5 shadow-md"
      data-testid="conversation-find-bar"
      role="search"
    >
      <div className="flex items-center gap-2">
        <Input
          ref={inputRef}
          data-testid="conversation-find-input"
          aria-label="Find in conversation"
          placeholder="Find in conversation..."
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          className="h-8 flex-1"
        />
        <Text
          type="secondary"
          className="min-w-14 text-center text-xs tabular-nums"
          aria-live="polite"
          data-testid="conversation-find-count"
        >
          {countLabel}
        </Text>
        <Button
          data-testid="conversation-find-prev"
          variant="ghost"
          size="icon"
          icon={<ChevronUp />}
          aria-label="Previous match"
          disabled={total === 0 || activeIndex === 0}
          onClick={() => void go(-1)}
        />
        <Button
          data-testid="conversation-find-next"
          variant="ghost"
          size="icon"
          icon={<ChevronDown />}
          aria-label="Next match"
          disabled={total === 0 || (activeIndex >= matches.length - 1 && !canLoadMore)}
          onClick={() => void go(1)}
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

      {matches.length > 0 && (
        <div
          ref={resultsRef}
          className="flex max-h-64 flex-col gap-0.5 overflow-y-auto"
          data-testid="conversation-find-results"
          role="group"
          aria-label="Search results"
          aria-busy={loading || undefined}
          onScroll={handleResultsScroll}
        >
          {matches.map((m, idx) => (
            <Button
              key={m.message_id}
              data-testid="conversation-find-result"
              variant="ghost"
              block
              aria-current={idx === activeIndex ? 'true' : undefined}
              onClick={() => {
                setActiveIndex(idx)
                void activateMatch(m)
              }}
              className={cn(
                'h-auto justify-start whitespace-normal px-2 py-1.5 text-start text-xs',
                idx === activeIndex && 'bg-accent',
              )}
            >
              <span
                className={cn(
                  'line-clamp-2',
                  idx === activeIndex
                    ? 'text-accent-foreground'
                    : 'text-muted-foreground',
                )}
              >
                {m.snippet}
              </span>
            </Button>
          ))}
          {/* Live region so SR users hear a further page loading. */}
          <div aria-live="polite" className="flex justify-center">
            {loading && (
              <div className="py-1">
                <Loader2
                  className="animate-spin"
                  aria-label="Loading more results"
                />
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
