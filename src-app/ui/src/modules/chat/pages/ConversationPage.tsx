import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useParams } from 'react-router-dom'
import type { OverlayScrollbarsComponentRef } from 'overlayscrollbars-react'
import { Alert, Button, ErrorState, Tooltip } from '@/components/ui'
import { Search as SearchIcon } from 'lucide-react'
import { Loading } from '@/core/components/Loading'
import { MessageList } from '@/modules/chat/components/MessageList'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { TitleEditor } from '@/modules/chat/components/TitleEditor'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { ChatRightPanel } from '@/modules/chat/core/components/ChatRightPanel'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { Stores } from '@/core'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { DivScrollY } from '@/components/common/DivScrollY'
import { cn } from '@/lib/utils'
import { ConversationFindBar } from '@/modules/chat/components/ConversationFindBar'
import { ConversationFindContext } from '@/modules/chat/components/ConversationFindContext'
import { JumpToLatestButton } from '@/modules/chat/components/JumpToLatestButton'
import {
  captureTopAnchor,
  measureMessageTop,
  restoreDelta,
  type ScrollAnchor,
} from '@/modules/chat/core/utils/scrollAnchor.utils'
import { firstMessageId } from '@/modules/chat/core/stores/messageWindow'

export default function ConversationPage() {
  const { conversationId } = useParams<{ conversationId: string }>()

  const { conversation, messages, loading, error, hasMoreBefore, hasMoreAfter } =
    Stores.Chat
  // Native document-scroll on mobile: the message history scrolls the WINDOW
  // (iOS toolbar collapses as you scroll up) while the composer stays pinned via
  // position:sticky. Desktop keeps the fixed inner-scroll shell. The right panel
  // is a fixed overlay on mobile, so the chat column is effectively full-width.
  useNativeScroll(true)
  const { nativeScroll } = Stores.AppLayout

  // Load conversation and messages on mount or when ID changes.
  useEffect(() => {
    if (conversationId) {
      Stores.Chat.loadConversation(conversationId)
    }
  }, [conversationId])

  // Auto-scroll to bottom on new messages, but ONLY if the user is
  // already near the bottom. Previously this scrolled unconditionally
  // on every message-map change, yanking the user back to the bottom
  // whenever they scrolled up to read history mid-stream. (audit 04 HIGH-3)
  //
  // Implementation: an IntersectionObserver watches the messagesEnd
  // sentinel. When it intersects the viewport (i.e., the user is at
  // the bottom), we set isAtBottomRef = true; when it leaves, false.
  // The scroll effect only fires when isAtBottomRef is true.
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const isAtBottomRef = useRef(true)
  // Conversation id whose initial bottom-jump we've already done.
  const initialScrollConvIdRef = useRef<string | null>(null)

  // ── Reverse-infinite-scroll (load older on scroll-up) refs (ITEM-9) ────────
  // The OverlayScrollbars component (desktop inner scroll). In mobile native
  // flow it renders a plain div and this stays null → we fall back to window.
  const scrollerRef = useRef<OverlayScrollbarsComponentRef>(null)
  // The message content container (holds every [data-message-id]).
  const messagesContainerRef = useRef<HTMLDivElement>(null)
  // Top sentinel — when it approaches the viewport top we prepend older msgs.
  const topSentinelRef = useRef<HTMLDivElement>(null)
  // Bottom sentinel — when it approaches the viewport bottom AND the window is
  // anchored mid-conversation (`hasMoreAfter`, e.g. after an around= jump) we
  // append the next NEWER page so the user can scroll DOWN toward the latest.
  const bottomLoadSentinelRef = useRef<HTMLDivElement>(null)
  // Anchor captured just before a prepend so we can re-pin the view after it.
  const pendingAnchorRef = useRef<{
    anchor: ScrollAnchor
    prevFirstId: string
  } | null>(null)
  // A short-lived ResizeObserver re-applies the restore as late async content
  // (images/katex/mermaid/shiki) in the prepended block resolves.
  const anchorResizeObsRef = useRef<ResizeObserver | null>(null)
  // Flips true once the OverlayScrollbars instance is initialized (desktop
  // inner scroll). The reverse-scroll observer keys on it so it re-creates with
  // the correct viewport `root` instead of the window fallback captured before
  // the scroller mounts. Never flips in mobile native-flow (no OS instance) —
  // there the window root is correct anyway.
  const [scrollerReady, setScrollerReady] = useState(false)

  // Resolve the active scroll viewport for both desktop (OverlayScrollbars) and
  // mobile (native window scroll). `viewportTop` is the client-Y of the viewport
  // top edge used to compute anchor offsets.
  const getViewport = useCallback((): {
    scrollBy: (delta: number) => void
    viewportTop: number
    root: HTMLElement | null
  } | null => {
    const os = scrollerRef.current?.osInstance()
    const vp = os?.elements().viewport as HTMLElement | undefined
    if (vp) {
      return {
        scrollBy: d => {
          vp.scrollTop += d
        },
        viewportTop: vp.getBoundingClientRect().top,
        root: vp,
      }
    }
    // Native window scroll (mobile nativeFlow).
    return {
      scrollBy: d => window.scrollBy(0, d),
      viewportTop: 0,
      root: null,
    }
  }, [])

  // In-conversation find (ITEM-1) + jump-to-latest visibility (ITEM-2).
  const [findOpen, setFindOpen] = useState(false)
  // Restore focus to the toggle when the find bar closes (it unmounts, so its
  // focused input would otherwise drop focus to <body>).
  const findToggleRef = useRef<HTMLButtonElement>(null)
  const closeFind = () => {
    setFindOpen(false)
    findToggleRef.current?.focus()
  }
  const [activeMatchId, setActiveMatchId] = useState<string | null>(null)
  // Mirror of isAtBottomRef surfaced to render so the jump-to-latest button can
  // show/hide. The ref stays the source of truth for the scroll effects.
  const [atBottom, setAtBottom] = useState(true)

  // Re-attach when the conversation becomes available: on first mount the
  // Loading / ErrorState early-returns render NO sentinel, so an empty-dep
  // effect would bail once and never observe. Keying on the loaded conversation
  // id re-runs the effect when the main view (and the sentinel) actually mounts.
  useEffect(() => {
    const sentinel = messagesEndRef.current
    if (!sentinel) return
    const observer = new IntersectionObserver(
      entries => {
        const intersecting = entries[0]?.isIntersecting ?? false
        isAtBottomRef.current = intersecting
        setAtBottom(intersecting)
      },
      { root: null, threshold: 0 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [conversation?.id])

  // Cmd/Ctrl-F opens the in-conversation find bar, overriding the browser's
  // native find (our find covers the same rendered message content — DEC-5).
  // Only when a conversation is actually loaded — otherwise the find bar isn't
  // rendered (Loading / ErrorState early returns), so we must NOT swallow native
  // find and strand the user with neither.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && !e.altKey && e.key.toLowerCase() === 'f') {
        if (!Stores.Chat.$.conversation) return
        e.preventDefault()
        setFindOpen(true)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  const findContextValue = useMemo(() => ({ activeMatchId }), [activeMatchId])

  const jumpToLatest = async () => {
    // If the window is anchored mid-conversation (after an around= jump), the
    // real latest message isn't loaded — `messagesEndRef` is only the bottom of
    // the loaded slice. Snap to the tail first so "Jump to latest" reaches the
    // actual latest, then scroll instantly (content just changed).
    const cid = Stores.Chat.$.conversation?.id
    if (cid && Stores.Chat.$.hasMoreAfter) {
      await Stores.Chat.loadMessages(cid)
      requestAnimationFrame(() =>
        messagesEndRef.current?.scrollIntoView({ behavior: 'auto' }),
      )
    } else {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }

  // Initial load: jump to the bottom INSTANTLY (before paint), once per
  // conversation. An animated scroll-through would drag the viewport past
  // every message and trigger lazy-loading of all inline file previews — the
  // instant jump means off-screen previews never enter the viewport, so they
  // stay un-fetched until scrolled to. useLayoutEffect runs before paint so
  // the first frame is already at the bottom.
  //
  // Gate on the STORE's loaded conversation matching the URL, not just
  // `messages.size`: on an in-app A→B switch the URL param flips to B before
  // `loadConversation` runs, so for one render `conversationId===B` while the
  // store still holds A's `conversation`/`messages`. Latching then would
  // consume the jump against A's stale content and leave B to an animated
  // smooth-scroll-through (defeating lazy-loading). Requiring
  // `conversation?.id === conversationId` makes the latch wait for B's data.
  useLayoutEffect(() => {
    if (!conversationId) return
    if (
      conversation?.id === conversationId &&
      messages.size > 0 &&
      initialScrollConvIdRef.current !== conversationId
    ) {
      initialScrollConvIdRef.current = conversationId
      // A `#message-<id>` deep-link handler (below) owns the initial scroll in
      // that case — don't yank the view to the bottom over it.
      if (!/^#message-/.test(window.location.hash)) {
        messagesEndRef.current?.scrollIntoView({ behavior: 'auto' })
      }
    }
  }, [conversationId, conversation, messages])

  // Subsequent message changes (e.g. streaming deltas): smooth-follow, but
  // only when the loaded conversation matches the URL, the initial jump for it
  // has happened, and the user is already at the bottom. The conversation gate
  // stops a smooth animation from firing during the stale A→B switch window.
  // Also suppressed while an older-page prepend is being scroll-anchored
  // (pendingAnchorRef) — otherwise the bottom-follow would fight the restore.
  // AND suppressed when `hasMoreAfter` is true: the loaded window is anchored
  // MID-conversation (after an around= jump), so `messagesEndRef` is not the
  // real latest — auto-following it would re-enter the bottom-load sentinel and
  // cascade an un-interruptible auto-scroll toward the tail. Auto-follow only
  // makes sense once the window actually holds the newest message.
  useEffect(() => {
    if (
      !pendingAnchorRef.current &&
      !hasMoreAfter &&
      conversation?.id === conversationId &&
      initialScrollConvIdRef.current === conversationId &&
      isAtBottomRef.current
    ) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages, conversationId, conversation, hasMoreAfter])

  // ── Reverse-infinite-scroll: load older on scroll-up (ITEM-9) ──────────────
  // A top sentinel with an 800px rootMargin (~1.5 viewports) prefetches the next
  // older page BEFORE the user reaches the very top. Just before dispatching we
  // capture the top-visible message as a scroll anchor so the prepend doesn't
  // teleport the view (restored in the layout effect below).
  useEffect(() => {
    const sentinel = topSentinelRef.current
    if (!sentinel) return
    const view = getViewport()
    const observer = new IntersectionObserver(
      async entries => {
        if (!entries[0]?.isIntersecting) return
        // Fresh store reads — the closure's `hasMoreBefore`/`loadingOlder` could
        // be stale between re-renders.
        if (!Stores.Chat.$.hasMoreBefore || Stores.Chat.$.loadingOlder) return
        const container = messagesContainerRef.current
        const v = getViewport()
        const prevFirstId = firstMessageId(Stores.Chat.$.messages)
        if (container && v && prevFirstId) {
          const anchor = captureTopAnchor(container, v.viewportTop)
          if (anchor) pendingAnchorRef.current = { anchor, prevFirstId }
        }
        await Stores.Chat.loadOlderMessages()
        // If NO older page landed (guard early-return, empty/duplicate page, or
        // a conversation switch), the restore layout-effect never fires — clear
        // the pending anchor so it can't stick truthy and permanently suppress
        // the bottom auto-follow. `$.messages` reflects the post-`set` store.
        const pending = pendingAnchorRef.current
        if (
          pending &&
          firstMessageId(Stores.Chat.$.messages) === pending.prevFirstId
        ) {
          pendingAnchorRef.current = null
        }
      },
      { root: view?.root ?? null, rootMargin: '800px 0px 0px 0px', threshold: 0 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
    // `scrollerReady` re-creates the observer once the OverlayScrollbars
    // viewport exists so `root` is the real scroll box, not the window fallback.
  }, [conversation?.id, hasMoreBefore, getViewport, scrollerReady])

  // Load NEWER messages on scroll-down when the window is anchored mid-
  // conversation (after an around= jump). No scroll anchoring needed: appending
  // below the fold doesn't shift what's already visible. The 800px bottom
  // rootMargin prefetches before the user hits the loaded slice's end.
  useEffect(() => {
    const sentinel = bottomLoadSentinelRef.current
    if (!sentinel) return
    const view = getViewport()
    const observer = new IntersectionObserver(
      entries => {
        if (!entries[0]?.isIntersecting) return
        if (!Stores.Chat.$.hasMoreAfter || Stores.Chat.$.isStreaming) return
        void Stores.Chat.loadNewerMessages()
      },
      { root: view?.root ?? null, rootMargin: '0px 0px 800px 0px', threshold: 0 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [conversation?.id, hasMoreAfter, getViewport, scrollerReady])

  // After an older page is PREPENDED, re-pin the captured anchor so the view
  // stays put (DEC-2). Runs before paint; a short-lived ResizeObserver re-applies
  // the correction as late async content (images/katex/mermaid/shiki) resolves.
  useLayoutEffect(() => {
    const pending = pendingAnchorRef.current
    if (!pending) return
    const currentFirst = firstMessageId(messages)
    // Only act once the prepend actually landed (oldest-loaded id changed).
    if (!currentFirst || currentFirst === pending.prevFirstId) return
    pendingAnchorRef.current = null

    const applyRestore = () => {
      const c = messagesContainerRef.current
      const v = getViewport()
      if (!c || !v) return
      const newTop = measureMessageTop(c, pending.anchor.anchorId)
      if (newTop == null) return
      const delta = restoreDelta(pending.anchor.savedTop, newTop - v.viewportTop)
      if (delta !== 0) v.scrollBy(delta)
    }
    applyRestore()

    // Re-apply for ≤1s as async heights settle, then disconnect. A user gesture
    // (wheel / touch / arrow keys) STOPS the re-pin immediately so it never
    // fights the user if they scroll away while late content is still resizing.
    // (Gesture events fire only on real input — not on our own programmatic
    // scrollBy — so applyRestore's scroll can't self-cancel.)
    anchorResizeObsRef.current?.disconnect()
    const container = messagesContainerRef.current
    const view = getViewport()
    const gestureTarget: EventTarget = view?.root ?? window
    let stopAt = 0
    const teardown = () => {
      if (stopAt) window.clearTimeout(stopAt)
      anchorResizeObsRef.current?.disconnect()
      anchorResizeObsRef.current = null
      gestureTarget.removeEventListener('wheel', teardown)
      gestureTarget.removeEventListener('touchmove', teardown)
      gestureTarget.removeEventListener('keydown', teardown)
    }
    if (container) {
      const ro = new ResizeObserver(() => applyRestore())
      ro.observe(container)
      anchorResizeObsRef.current = ro
      stopAt = window.setTimeout(teardown, 1000)
      gestureTarget.addEventListener('wheel', teardown, { passive: true })
      gestureTarget.addEventListener('touchmove', teardown, { passive: true })
      gestureTarget.addEventListener('keydown', teardown)
    }
    return teardown
  }, [messages, getViewport])

  // Clean up the anchor ResizeObserver on unmount.
  useEffect(() => {
    return () => anchorResizeObsRef.current?.disconnect()
  }, [])

  // ── Deep-link: `#message-<id>` jumps to a (possibly-unloaded) message ───────
  // Consumed by citations / cross-surface links. Loads a window centered on the
  // message (around=), centers it, and highlights it via the find ring.
  useEffect(() => {
    if (!conversation?.id) return
    let cleared: number | undefined
    const applyHash = async () => {
      const m = window.location.hash.match(/^#message-(.+)$/)
      if (!m) return
      const messageId = m[1]
      const found = await Stores.Chat.jumpToMessage(messageId)
      if (!found || Stores.Chat.$.conversation?.id !== conversation.id) return
      // Wait for the centered window to render, then center + highlight.
      requestAnimationFrame(() => {
        document
          .querySelector(`[data-message-id="${CSS.escape(messageId)}"]`)
          ?.scrollIntoView({ behavior: 'auto', block: 'center' })
        setActiveMatchId(messageId)
        if (cleared) window.clearTimeout(cleared)
        cleared = window.setTimeout(() => setActiveMatchId(null), 2500)
      })
    }
    void applyHash()
    window.addEventListener('hashchange', applyHash)
    return () => {
      window.removeEventListener('hashchange', applyHash)
      if (cleared) window.clearTimeout(cleared)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [conversation?.id])

  // Loading state
  if (loading && !conversation) {
    return (
      <Loading />
    )
  }

  // No conversation to show. A load FAILURE (the store set `error`) is a
  // transient/permission problem → offer a persistent retry, not the misleading
  // "deleted" copy. A clean miss (no error) is a genuine not-found.
  if (!loading && !conversation) {
    return (
      <div className="flex flex-col items-center justify-center h-full p-8">
        {error ? (
          <ErrorState
            resource="conversation"
            description="This conversation couldn't be loaded. Check your connection and try again."
            details={error}
            onRetry={() =>
              conversationId && Stores.Chat.loadConversation(conversationId)
            }
            className="max-w-md"
            data-testid="chat-conversation-error"
          />
        ) : (
          <Alert
            data-testid="chat-conversation-not-found-alert"
            tone="error"
            title="Conversation not found"
            description="This conversation may have been deleted or you don't have access to it."
          />
        )}
      </div>
    )
  }

  return (
    <div className={cn('flex flex-col', nativeScroll ? 'min-h-dvh' : 'h-full')}>
      {/* Header — full width, matches the rest of the app's header bars
          (project page, settings, etc.). TitleEditor at the left,
          slot consumers at the right. */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full gap-2">
          <div className="flex items-center min-w-0 gap-2">
            <TitleEditor />
          </div>
          <div className="flex items-center gap-1">
            {/* Find-in-conversation toggle (ITEM-1). Also openable via
                Cmd/Ctrl-F. */}
            <Tooltip content="Find in conversation">
              <Button
                ref={findToggleRef}
                data-testid="conversation-find-toggle-btn"
                variant={findOpen ? 'default' : 'ghost'}
                size="icon"
                icon={<SearchIcon />}
                aria-label="Find in conversation"
                aria-pressed={findOpen}
                onClick={() => (findOpen ? closeFind() : setFindOpen(true))}
              />
            </Tooltip>
            {/* Decoupled chip injection point — other modules register
                header decorations into `chatConversationHeaderTrailing`
                without chat compiling against them. */}
            <ConversationHeaderTrailingSlot />
          </div>
        </div>
      </HeaderBarContainer>

      {/* Error banner */}
      {error && (
        <div className="w-full max-w-4xl mx-auto px-4 pt-4">
          <Alert data-testid="chat-conversation-error-alert" tone="error" title={error} onClose={Stores.Chat.clearError} closeLabel="Close" />
        </div>
      )}

      {/* Main area: chat column + right panel */}
      <div className={cn('flex flex-1 min-h-0', nativeScroll ? '' : 'overflow-hidden')}>
        {/* Chat column. `relative` anchors the floating find bar (ITEM-1) and
            jump-to-latest button (ITEM-2). */}
        <div className={cn('relative flex flex-col flex-1 min-w-0', nativeScroll ? '' : 'overflow-hidden')}>
          {/* Floating find bar, top-right of the chat column. */}
          <div className="pointer-events-none absolute end-3 top-3 z-30 flex justify-end">
            <div className="pointer-events-auto">
              <ConversationFindBar
                open={findOpen}
                onClose={closeFind}
                onActiveMatchChange={setActiveMatchId}
              />
            </div>
          </div>
          {/* Pinned conversation-context chrome (the "In project" chip and any
              mode/model indicators registered into message_list_header). It's a
              SIBLING above the message scroll container — never a descendant of
              it — so it can't scroll out of view (K1/K4). Desktop: only the inner
              list scrolls, so normal flow already pins this. Mobile (native
              document-scroll): stick it to the top so it survives scroll-to-
              bottom. Renders nothing (zero height) when the conversation is
              unfiled, so there's no empty bar. */}
          <div
            className={cn(
              'w-full max-w-4xl mx-auto',
              nativeScroll ? 'sticky top-0 z-20 bg-background' : '',
            )}
            data-testid="conversation-context-chrome"
          >
            <ExtensionSlot name="message_list_header" />
          </div>
          {/* Desktop: overlay scroll (DivScrollY / OverlayScrollbars) so the
              message history matches every other scroll surface in the app
              instead of a heavy native scrollbar. `nativeFlow` keeps the mobile
              window-scroll path (sticky composer, iOS toolbar collapse)
              unchanged — it renders a plain flow container when nativeScroll is
              active, exactly like the previous conditional did. The
              messagesEnd sentinel + scrollIntoView + isAtBottom observer all
              keep working: OverlayScrollbars scrolls a real viewport element. The
              context chrome above stays a SIBLING of this scroller (K1/K4). */}
          <DivScrollY
            nativeFlow
            className="flex-1"
            ref={scrollerRef}
            events={{ initialized: () => setScrollerReady(true) }}
          >
            {/* `overflow-anchor: none` stops the browser's own scroll anchoring
                from fighting the manual anchor restore on prepend (DEC-2). */}
            <div
              ref={messagesContainerRef}
              className="w-full max-w-4xl mx-auto px-4 pt-4"
              style={{ overflowAnchor: 'none' }}
            >
              {/* Top sentinel: intersecting (with an 800px rootMargin) triggers
                  loading the next older page (reverse infinite scroll). */}
              <div
                ref={topSentinelRef}
                aria-hidden="true"
                data-testid="chat-top-sentinel"
              />
              <ConversationFindContext.Provider value={findContextValue}>
                <MessageList />
              </ConversationFindContext.Provider>
              {/* Bottom-load sentinel: triggers loading NEWER messages when the
                  window is anchored mid-conversation (after an around= jump). */}
              <div
                ref={bottomLoadSentinelRef}
                aria-hidden="true"
                data-testid="chat-bottom-load-sentinel"
              />
              <div ref={messagesEndRef} />
            </div>
          </DivScrollY>
          {/* Composer: pinned. Native mode → position:sticky at the viewport
              bottom (with home-indicator safe-area) so messages document-scroll
              underneath; desktop → normal flow at the column bottom. Made a
              positioning context (sticky in native, relative on desktop) so the
              jump-to-latest button can anchor to its TOP edge. */}
          <div
            className={cn(
              'w-full max-w-4xl mx-auto p-4',
              nativeScroll ? 'sticky bottom-0 z-10 bg-background' : 'relative',
            )}
            style={
              nativeScroll
                ? { paddingBottom: 'calc(env(safe-area-inset-bottom, 0px) + 16px)' }
                : undefined
            }
          >
            {/* Jump-to-latest: floats just ABOVE the composer (bottom-full anchors
                it to the composer's top edge, so it clears the input regardless of
                the input's height). Shown only when scrolled up (ITEM-2). */}
            <div className="pointer-events-none absolute inset-x-0 bottom-full mb-3 z-20 flex justify-center">
              <div className="pointer-events-auto">
                <JumpToLatestButton visible={!atBottom} onClick={jumpToLatest} />
              </div>
            </div>
            <ChatInput />
          </div>
        </div>

        {/* Right sidebar panel */}
        <ChatRightPanel />
      </div>
    </div>
  )
}

/**
 * Consumes the `chatConversationHeaderTrailing` slot. Other modules
 * register header decorations here; chat doesn't compile against
 * any of them.
 */
function ConversationHeaderTrailingSlot() {
  const { slots } = Stores.ModuleSystem
  const rawItems = slots.get('chatConversationHeaderTrailing')
  // Memoize the sorted copy — the underlying slot array is stable
  // (mutates only on module-registration changes, which don't happen
  // post-bootstrap), so this collapses to a one-time sort.
  const items = useMemo(
    () =>
      (rawItems || [])
        .slice()
        .sort((a, b) => (a.order ?? 0) - (b.order ?? 0)),
    [rawItems],
  )
  return (
    <>
      {items.map(item => (
        <LazyComponentRenderer key={item.id} component={item.component} />
      ))}
    </>
  )
}
