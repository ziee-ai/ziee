import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useParams } from 'react-router-dom'
import type { OverlayScrollbarsComponentRef } from 'overlayscrollbars-react'
import { Alert, Button, ErrorState, Tooltip } from '@ziee/kit'
import { Search as SearchIcon } from 'lucide-react'
import { Loading } from '@/core/components/Loading'
import {
  MessageList,
  type MessageAnchor,
  type MessageListHandle,
} from '@/modules/chat/components/MessageList'
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
  // Live MCP tool-call statuses — subscribed reactively (proxy destructure) so a
  // newly-`pending_approval` tool triggers the scroll-to-approval effect below.
  const { toolCalls } = Stores.McpComposer

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
  // Drive the right panel's drawer-vs-side-panel by the conversation area's OWN
  // width (page size, sidebar-aware), not the window — so with the sidebar open
  // on a wide window the (now narrow) page still gets the overlay drawer.
  const mainAreaRef = useRef<HTMLDivElement>(null)
  const [rightPanelNarrow, setRightPanelNarrow] = useState(false)
  // Small-screen (native document-scroll) composer auto-hide: hide it when the
  // user scrolls UP into older messages (more reading room), reveal it when they
  // scroll DOWN toward newer messages / the bottom.
  const [composerHidden, setComposerHidden] = useState(false)
  // Conversation id whose initial bottom-jump we've already done.
  const initialScrollConvIdRef = useRef<string | null>(null)
  // tool_use_ids whose pending approval we've already scrolled to — so the
  // scroll-to-approval effect fires once per approval, not on every toolCalls change.
  const scrolledApprovalsRef = useRef<Set<string>>(new Set())

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
  // Imperative handle to the virtualized MessageList (scroll-to + anchor).
  const messageListRef = useRef<MessageListHandle>(null)
  // Anchor captured just before a prepend so we can re-pin the view after it
  // (index-based, via the virtualizer — see MessageList.restoreAnchor).
  const pendingAnchorRef = useRef<{
    anchor: MessageAnchor
    prevFirstId: string
  } | null>(null)
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

  // Measure the conversation area's width for the right-panel drawer decision.
  // Keyed on the loaded conversation id for the SAME reason as the sentinel
  // observer above: the main area only mounts once the Loading/Error early
  // returns clear, so an empty-dep effect would bail with a null ref and the
  // width would stay at its (narrow) initial value.
  useEffect(() => {
    const el = mainAreaRef.current
    if (!el) return
    // md breakpoint (≤768px page width) — below it the side panel is too cramped
    // next to the messages, so the panel opens as a Drawer instead.
    const measure = () =>
      setRightPanelNarrow(el.getBoundingClientRect().width <= 768)
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [conversation?.id])

  // Composer auto-hide on scroll direction (native document-scroll only — the
  // mobile mode). Overscroll-guarded + debounced like the header so an iOS
  // rubber-band bounce can't flip-flop it.
  useEffect(() => {
    if (!nativeScroll) {
      setComposerHidden(false)
      return
    }
    let lastY = window.scrollY
    let lastToggle = 0
    const onScroll = () => {
      const y = window.scrollY
      const maxY = document.documentElement.scrollHeight - window.innerHeight
      if (y < 0 || y > maxY) {
        lastY = Math.max(0, Math.min(y, maxY)) // ignore rubber-band overscroll
        return
      }
      if (maxY - y <= 8) {
        setComposerHidden(false) // at the newest message → always show
        lastY = y
        return
      }
      const dy = y - lastY
      if (Math.abs(dy) < 6) return // jitter
      const now = performance.now()
      if (now - lastToggle >= 250) {
        lastToggle = now
        setComposerHidden(dy < 0) // scrolling up → hide; down → show
      }
      lastY = y
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [nativeScroll])

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
        // The messagesEndRef jump lands on ESTIMATED row heights under
        // virtualization; settle on the true (measured) bottom.
        messageListRef.current?.scrollToBottom()
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

  // On mount, treat any already-`pending_approval` tool calls as already-scrolled.
  // `toolCalls` is a process-global map that is never cleared across conversations,
  // so a leftover pending approval from a previously-viewed conversation must NOT
  // yank a freshly-opened one to the bottom — only approvals that appear AFTER this
  // page mounts should scroll.
  useEffect(() => {
    for (const [id, call] of toolCalls) {
      if (call.status === 'pending_approval') scrolledApprovalsRef.current.add(id)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // A tool call that needs approval must grab the user's attention even if they've
  // scrolled up reading history — the streaming approval is injected at the TAIL of
  // the message, so bring the tail into view. Same guards as the auto-follow effect
  // above, EXCEPT the `isAtBottomRef` gate is deliberately bypassed (that gate is
  // exactly why an off-bottom approval was never scrolled to). Fires once per
  // approval (deduped). `messagesEndRef.scrollIntoView` moves the native/mobile
  // (non-virtualized) scroll and gets close under virtualization (estimated
  // heights); `messageListRef.scrollToBottom()` then settles on the true measured
  // bottom of the virtualized list (a no-op on the mobile plain path).
  useEffect(() => {
    // Record every newly-pending approval as "seen" REGARDLESS of whether we
    // scroll to it — so an approval that arrives while a guard is active (e.g. a
    // mid-list `hasMoreAfter` window) is not left un-seen and cannot trigger a
    // stray cross-conversation scroll later.
    const seen = scrolledApprovalsRef.current
    let hasNewApproval = false
    for (const [id, call] of toolCalls) {
      if (call.status === 'pending_approval' && !seen.has(id)) {
        seen.add(id)
        hasNewApproval = true
      }
    }
    if (!hasNewApproval) return
    // Only scroll when it's safe — same guards as the auto-follow effect above,
    // EXCEPT the `isAtBottomRef` gate is deliberately bypassed (that gate is
    // exactly why an off-bottom approval was never scrolled to).
    if (
      pendingAnchorRef.current ||
      hasMoreAfter ||
      conversation?.id !== conversationId ||
      initialScrollConvIdRef.current !== conversationId
    ) {
      return
    }
    // Desktop is virtualized → `scrollToBottom()` (virt.scrollToIndex + re-assert)
    // is what actually moves the OverlayScrollbars viewport. On the mobile plain
    // path `scrollToBottom()` is a no-op, so fall back to the end-anchor
    // `scrollIntoView` (which moves the native document scroll). Splitting on
    // `nativeScroll` avoids running BOTH on desktop, where the anchor jump would
    // fight the virtualized scroll.
    if (nativeScroll) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'auto' })
    } else {
      messageListRef.current?.scrollToBottom()
    }
  }, [toolCalls, conversation, conversationId, hasMoreAfter])

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
        // Capture the top-visible message as an index anchor via the virtualizer
        // so the prepend doesn't teleport the view (restored below).
        const prevFirstId = firstMessageId(Stores.Chat.$.messages)
        const anchor = messageListRef.current?.captureAnchor() ?? null
        if (anchor && prevFirstId) {
          pendingAnchorRef.current = { anchor, prevFirstId }
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
  // stays put (ITEM-4). Runs before paint. Index-based via the virtualizer,
  // which then settles the estimate→measured height corrections of the
  // prepended rows itself (shouldAdjustScrollPositionOnItemSizeChange).
  useLayoutEffect(() => {
    const pending = pendingAnchorRef.current
    if (!pending) return
    const currentFirst = firstMessageId(messages)
    // Only act once the prepend actually landed (oldest-loaded id changed).
    if (!currentFirst || currentFirst === pending.prevFirstId) return
    pendingAnchorRef.current = null
    messageListRef.current?.restoreAnchor(pending.anchor)
  }, [messages])

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
      // Wait for the centered window to render, then scroll the (possibly
      // virtualized-out) target into view via the virtualizer + highlight.
      requestAnimationFrame(() => {
        messageListRef.current?.scrollToMessageId(messageId, 'center')
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
      <div ref={mainAreaRef} className={cn('flex flex-1 min-h-0', nativeScroll ? '' : 'overflow-hidden')}>
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
                scrollToMessage={id =>
                  messageListRef.current?.scrollToMessageId(id, 'center') ?? false
                }
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
              nativeScroll ? 'sticky top-0 z-20 bg-card' : '',
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
                from fighting the manual anchor restore on prepend (DEC-2).
                pb-8 matches the composer's h-8 fade so the last message can
                scroll clear of it at rest (fully readable, not dissolved). */}
            <div
              ref={messagesContainerRef}
              className="w-full max-w-4xl mx-auto px-4 pt-4 pb-8"
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
                <MessageList
                  ref={messageListRef}
                  getScrollElement={() => getViewport()?.root ?? null}
                  scrollerReady={scrollerReady}
                  // Desktop (inner OS scroll) virtualizes; mobile native
                  // window-scroll renders the bounded window plainly.
                  virtualize={!nativeScroll}
                />
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
          {/* Bottom backdrop — the mirror of HeaderBarContainer's z-29 top panel.
              The composer sticks at bottom:5 (not 0) to dodge iOS Safari's
              bottom-edge sticky latch (same reason the header uses top:5), which
              leaves a 5px gap below it; this fixed opaque panel fills that gap +
              the home-indicator safe-area so document-scrolling content can flow
              behind the iOS bottom navigation bar without peeking through. Only
              while the composer is shown (mirrors the header's `pinned &&`). */}
          {nativeScroll && !composerHidden && (
            <div
              aria-hidden
              className="fixed inset-x-0 bottom-0 bg-card animate-in fade-in duration-300"
              style={{ height: 'calc(env(safe-area-inset-bottom, 0px) + 5px)', zIndex: 9 }}
            />
          )}
          {/* Composer: pinned. Native mode → position:sticky at bottom:5 (with
              home-indicator safe-area) so messages document-scroll underneath;
              desktop → normal flow at the column bottom. Made a positioning
              context (sticky in native, relative on desktop) so the jump-to-latest
              button can anchor to its TOP edge. */}
          <div
            className={cn(
              // pt-0: no gap above the input — the fade below stands in for it.
              'w-full max-w-4xl mx-auto p-4 pt-0',
              nativeScroll
                ? cn(
                    'bg-card',
                    // Auto-hide on scroll: toggle sticky↔relative (NOT a
                    // transform). Hidden → position:relative so the composer
                    // wipes away with the page as the user reads history; shown →
                    // sticky, pinned to the viewport bottom and sliding back in.
                    composerHidden
                      ? 'relative'
                      : 'sticky z-10 animate-in fade-in slide-in-from-bottom-4 duration-300 ease-out',
                  )
                : 'relative',
            )}
            style={
              nativeScroll
                ? {
                    // bottom:5 dodges iOS Safari's bottom sticky-latch (mirrors the
                    // header's top:5). The 5px is subtracted from paddingBottom so
                    // the input keeps its resting position (safe-area + 16) — same
                    // padding-compensation the header does for its 5px offset. In
                    // the relative (hidden) state, replicate the offset via
                    // marginBottom so it doesn't jump 5px when toggling.
                    paddingBottom: 'calc(env(safe-area-inset-bottom, 0px) + 11px)',
                    ...(composerHidden ? { marginBottom: 5 } : { bottom: 5 }),
                  }
                : undefined
            }
          >
            {/* Gradient fade above the composer: the tail of the message history
                dissolves into the surface (bg-card) as it scrolls up, instead of
                hard-cutting at the input's top edge. The message list carries a
                matching bottom pad so the last message clears this at rest. */}
            <div
              className={cn(
                'pointer-events-none absolute inset-x-0 bottom-full h-8 bg-gradient-to-t from-card to-transparent',
                // The fade belongs to the pinned composer — when the composer
                // wipes away (scrolled up into history) hide it too.
                nativeScroll && composerHidden && 'hidden',
              )}
            />
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
        <ChatRightPanel narrow={rightPanelNarrow} />
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
