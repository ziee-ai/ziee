import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import type { OverlayScrollbarsComponentRef } from 'overlayscrollbars-react'
import { Alert, Button, ErrorState, Tooltip } from '@ziee/kit'
import { Columns2, GripVertical, Search as SearchIcon, X } from 'lucide-react'
import { Loading } from '@/core/components/Loading'
import {
  MessageList,
  type MessageAnchor,
  type MessageListHandle,
} from '@/modules/chat/components/MessageList'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { ConversationPickerPane } from '@/modules/chat/components/ConversationPickerPane'
import { TitleEditor } from '@/modules/chat/components/TitleEditor'
import {
  useClosePane,
  useOpenConversationInWorkspace,
} from '@/modules/chat/core/pane/useOpenConversation'
import {
  dragKind,
  readConversationDragId,
  readPaneDragId,
  reorderIndices,
  setPaneDragData,
} from '@/modules/chat/core/pane/paneDnd'
import {
  type DropZone,
  planSinglePaneDrop,
  planSplitPaneDrop,
  zoneForX,
} from '@/modules/chat/core/pane/singlePaneDrop'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import { useConversationTearOff } from '@/modules/chat/core/popout/useConversationTearOff'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { useHeaderLeftInset } from '@/modules/layouts/app-layout/hooks/useHeaderLeftInset'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { ChatRightPanel } from '@/modules/chat/core/components/ChatRightPanel'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { DivScrollY } from '@/components/common/DivScrollY'
import { cn } from '@/lib/utils'
import { ConversationFindBar } from '@/modules/chat/components/ConversationFindBar'
import { ConversationFindContext } from '@/modules/chat/components/ConversationFindContext'
import { JumpToLatestButton } from '@/modules/chat/components/JumpToLatestButton'
import { firstMessageId } from '@/modules/chat/core/stores/messageWindow'
import { pendingApprovalIdsInPane } from '@/modules/chat/core/utils/toolCallPaneScope'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { useIsPopoutWindow } from '@/modules/chat/core/popout/useIsPopoutWindow'
import { SplitChatView } from '@/modules/chat/components/SplitChatView'
import { PaneManagerDrawer } from '@/modules/chat/components/PaneManagerDrawer'
import { McpComposer as McpComposerStore } from '@/modules/mcp/stores/mcpComposer'
import { AppLayout } from '@/modules/layouts/app-layout/appLayout'
import { SplitView as SplitViewStore } from '@/modules/chat/core/stores/splitView'
import { Chat as ChatStore } from '@/modules/chat/core/stores/chatBridge'
import { ModuleSystem } from '@ziee/framework/stores'

/**
 * Chat route element for `/chat/:conversationId`.
 *
 * Single-pane (0–1 split panes) → the normal `ConversationPane` bound to the URL
 * conversation via `ChatStore` (the primary pane). Once ≥2 split panes exist it
 * renders `SplitChatView`, which mounts one `ConversationPane` per pane inside a
 * `ChatPaneProvider`. Branching here on a single reactive read keeps hook order
 * stable; each branch is its own component boundary.
 */
export default function ConversationPage() {
  const { conversationId } = useParams<{ conversationId: string }>()
  const { panes, focusedPaneId } = SplitViewStore
  const navigate = useNavigate()
  // The FOCUSED pane's conversation (reactive) — what the URL must mirror while a
  // split is open.
  const focusedConvId =
    panes.find((p) => p.paneId === focusedPaneId)?.conversationId ?? null

  // URL → workspace reconcile (ITEM-25). The URL is a *view into* the workspace
  // (the focused pane). When it changes to a conversation NOT already shown by
  // the focused pane while a split is open — a deep link, a browser back/forward,
  // a "New chat in project" redirect — reconcile it in (focus its pane if open,
  // else replace the focused pane). Loop-guarded on the focused pane's current
  // conversation so the navigate that FOLLOWS a sidebar-click reconcile (which
  // already set the focused pane) does not re-trigger a second reconcile.
  useEffect(() => {
    if (!conversationId) return
    const sv = SplitViewStore.$
    if (sv.panes.length < 2) return // single-pane: the URL drives ConversationPane
    const focused = sv.panes.find((p) => p.paneId === sv.focusedPaneId)
    if (focused?.conversationId === conversationId) return // already shown → no-op
    SplitViewStore.openConversationInWorkspace(conversationId, 'auto')
  }, [conversationId])

  // Workspace → URL (FB-19). The MISSING second direction: opening a pane via the
  // Split button / an edge-drop / a picker, OR clicking a different pane to focus
  // it, changes the focused conversation but flows through `openPane`/`focusPane`
  // — neither of which navigates. Only the sidebar-open hook navigated, so the URL
  // got stuck on the FIRST conversation. That stale URL is why "open in new tab"
  // reopened the already-showing conversation ("the current rendering one") instead
  // of the focused one. Keep the URL in lockstep with the focused pane here:
  // `replace` so a focus change doesn't spam history, and the equality guard makes
  // it a strict no-op when the URL already matches (so it can never ping-pong with
  // the URL→workspace reconcile above). Deps are ONLY [focusedConvId, panes.length]
  // — NOT conversationId — so an external URL change (deep link) is handled by the
  // reconcile above rather than fought by this effect.
  useEffect(() => {
    // `=== 0` (not `< 2`): a PURE single-pane view (no SplitView pane) is URL-driven,
    // so skip. But a workspace COLLAPSING to a single surviving pane (e.g. the other
    // pane's conversation was deleted — FB-25) must still point the URL at the
    // survivor, or the single-pane view keeps loading the now-gone URL conversation
    // and toasts "conversation does not exist".
    if (panes.length === 0) return
    if (!focusedConvId || focusedConvId === conversationId) return
    navigate(`/chat/${focusedConvId}`, { replace: true })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusedConvId, panes.length])

  // The small-screen pane manager is mounted ONCE here (outside the pane subtrees)
  // so it's reachable from both the single-pane header and every split pane header,
  // and its global open-state survives the single-pane ⇄ split brancher swap below.
  return (
    <>
      {panes.length >= 2 ? <SplitChatView /> : <ConversationPane />}
      <PaneManagerDrawer />
    </>
  )
}

/**
 * One conversation surface — message history + composer + right panel.
 *
 * Rendered in TWO contexts: as the single-pane route (no `ChatPaneProvider` →
 * `pane` is null → drives the primary `ChatStore`), and as a pane inside
 * `SplitChatView` (wrapped in a provider → `pane` set → drives that pane's own
 * store). All store access goes through `chat`, so single-pane stays
 * byte-identical to before the split existed.
 */
export function ConversationPane() {
  const { conversationId: routeConversationId } = useParams<{
    conversationId: string
  }>()
  const pane = useChatPaneOrNull()
  const closePane = useClosePane()
  const openConversationInWorkspace = useOpenConversationInWorkspace()
  // Tear-off (ITEM-58): dragging this pane's grip past the window edge opens the
  // conversation as its own desktop window (and MOVES the pane out). No-op on web.
  const tearOff = useConversationTearOff()
  // The pop-out window is a focused single-conversation view — hide window-management
  // chrome (split, here; back + pop-out elsewhere) that only fits the main window
  // (ITEM-56 / FB-13).
  const isPopoutWindow = useIsPopoutWindow()
  // Pane header drop-zone (ITEM-31): a conversation dropped on the header replaces
  // this pane; a pane-header drag dropped here reorders. File drags are ignored
  // (they belong to the composer) — the header is not over the composer, so they
  // never cross-fire. `paneDropActive` drives the drop highlight.
  // `paneDropActive` = a pane-REORDER drag is over this pane's header (ring
  // highlight). The unified `onPaneArea*` handlers (defined below, after
  // `conversationId`) drive both this and the conversation edge-directional drop.
  const [paneDropActive, setPaneDropActive] = useState(false)
  // In a pane the provider owns the target conversation id (the URL param is the
  // route's, not this pane's); on the single-pane route it's the URL param.
  const conversationId = pane
    ? pane.conversationId ?? undefined
    : routeConversationId
  // Uniform store handle: the pane's own store in split, else the focused-pane
  // bridge (= primary) on the single-pane route. Both proxies expose the same
  // reactive-read / `.$` snapshot / action surface.
  const chat = (pane?.store ?? ChatStore) as typeof ChatStore
  // Split per-pane header must match the single-pane app header `HeaderBarContainer`
  // (ITEM-71 / FB-18): same 50px height, and the LEFTMOST pane reserves the same
  // left inset (shared `useHeaderLeftInset` — web 48/12, macOS-desktop 118) so its
  // content clears the fixed sidebar-collapse toggle + the macOS traffic lights.
  const headerLeftInset = useHeaderLeftInset()
  // Small screen (≤768px): a split pane can't tile columns, so it renders as a
  // normal single-pane conversation (normal header, no grip, no drag, no per-pane
  // ✕) and pane management moves to the `PaneManagerDrawer` (FB-26). `md === true`
  // is the drawer/single-visible-pane mode; desktop (false) keeps the compact
  // split header + drag-to-split + one-click split.
  const { md } = useWindowMinSize()
  // Read `panes` UNCONDITIONALLY (reactive) then derive — never behind `&&`, so the
  // reactive-proxy hook it triggers isn't conditional (Rules of Hooks; the file's
  // convention is `.$` snapshots for hook-free reads, reactive reads only at top level).
  const { panes: splitViewPanes, focusedPaneId: splitFocusedPaneId } =
    SplitViewStore
  const isLeftmostPane = !!pane && splitViewPanes[0]?.paneId === pane.paneId
  // On a small screen the FOCUSED pane is the only visible one; it should read like
  // a normal single-pane conversation — native document-scroll + the SAME auto-hiding
  // `HeaderBarContainer` (FB-28), so it inherits that header's real notch handling
  // (sticky top:5, the safe-area backdrop, relative-wipe) instead of a re-derived
  // approximation. `useMobileShell` marks that pane → it renders the
  // `HeaderBarContainer` branch below; hidden + desktop panes keep the compact header.
  // Safe to branch on because the store-proxy flag it depends on is read
  // unconditionally (a conditional proxy read would be a conditional hook).
  const isFocusedPane = !!pane && splitFocusedPaneId === pane.paneId
  const useMobileShell = !!pane && md && isFocusedPane
  // Per-pane edge-directional drop (ITEM-57 single-pane + ITEM-70 split): a
  // conversation dragged over a pane's chat column highlights the left/center/
  // right third. SINGLE-pane: left/right create the split ([dropped|current] /
  // [current|dropped]), center replaces in place. SPLIT: left/right insert a NEW
  // pane immediately before/after THIS pane, center replaces THIS pane; at
  // MAX_PANES the edges fall back to replace. Only conversation drags participate
  // — a file drag belongs to the composer; a pane-reorder drag is the header's.
  const [dropZone, setDropZone] = useState<DropZone | null>(null)
  // Unified pane-area drop, attached to BOTH the pane HEADER and the chat COLUMN
  // (they're SIBLINGS, so a header drop can't bubble to the column — blind-audit
  // fix). Dispatches by drag kind: a CONVERSATION drag drives the edge-directional
  // L/C/R zone (the whole pane is one target); a pane-REORDER drag highlights the
  // header + reorders. Both use `e.currentTarget`'s rect — header + column share
  // the pane's full width, so the x-fraction is consistent wherever you drop.
  const onPaneAreaDragOver = (e: React.DragEvent) => {
    const kind = dragKind(e.dataTransfer)
    if (kind === 'conversation') {
      if (!conversationId) return
      e.preventDefault()
      const rect = e.currentTarget.getBoundingClientRect()
      setDropZone(zoneForX(e.clientX, rect.left, rect.width))
    } else if (kind === 'pane' && pane) {
      e.preventDefault()
      setPaneDropActive(true)
    }
  }
  const onPaneAreaDragLeave = (e: React.DragEvent) => {
    // Ignore leave events that cross into a child; only clear on a real exit.
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
      setDropZone(null)
      setPaneDropActive(false)
    }
  }
  const onPaneAreaDrop = (e: React.DragEvent) => {
    setDropZone(null)
    setPaneDropActive(false)
    const kind = dragKind(e.dataTransfer)
    if (kind === 'pane') {
      if (!pane) return
      const from = readPaneDragId(e.dataTransfer)
      const idx = from ? reorderIndices(SplitViewStore.$.panes, from, pane.paneId) : null
      if (idx) {
        e.preventDefault()
        SplitViewStore.reorderPanes(idx.from, idx.to)
      }
      return
    }
    if (kind !== 'conversation' || !conversationId) return
    const droppedId = readConversationDragId(e.dataTransfer)
    if (!droppedId) return
    e.preventDefault()
    const rect = e.currentTarget.getBoundingClientRect()
    const zone = zoneForX(e.clientX, rect.left, rect.width)
    if (pane) {
      // Existing split: insert before/after THIS pane, or replace it.
      const atCap = SplitViewStore.$.panes.length >= SPLIT_LIMITS.MAX_PANES
      const plan = planSplitPaneDrop(zone, conversationId, droppedId, atCap)
      if (plan.kind === 'replace') {
        SplitViewStore.setPaneConversation(pane.paneId, droppedId)
      } else if (plan.kind === 'insertBefore') {
        SplitViewStore.openPane({ conversationId: droppedId, beforePaneId: pane.paneId })
      } else if (plan.kind === 'insertAfter') {
        SplitViewStore.openPane({ conversationId: droppedId, afterPaneId: pane.paneId })
      }
      return
    }
    // Single-pane: create the split (or replace in place). Route BOTH edges
    // through the canonical reconcile open (blind-audit fixes): `newPane` seeds
    // `[current | dropped]`, navigates to + focuses the DROPPED conversation, and
    // dedups a conversation already live in a pop-out WINDOW. A left drop then
    // reorders the dropped pane to the front.
    const plan = planSinglePaneDrop(zone, conversationId, droppedId)
    if (plan.kind === 'replace') {
      void openConversationInWorkspace(plan.id)
    } else if (plan.kind === 'split') {
      const droppedOnLeft = plan.order[0] === droppedId
      void openConversationInWorkspace(droppedId, { intent: 'newPane' }).then(() => {
        if (!droppedOnLeft) return
        const panes = SplitViewStore.$.panes
        const idx = panes.findIndex(p => p.conversationId === droppedId)
        if (idx > 0) SplitViewStore.reorderPanes(idx, 0)
      })
    }
  }

  const { conversation, messages, loading, error, hasMoreBefore, hasMoreAfter } =
    chat
  // Native document-scroll on mobile: the message history scrolls the WINDOW
  // (iOS toolbar collapses as you scroll up) while the composer stays pinned via
  // position:sticky. Desktop keeps the fixed inner-scroll shell.
  //   • Single-pane route → THIS component owns the flag (`useNativeScroll(!pane)`).
  //   • Split → `SplitChatView` owns it for the whole mobile split; a pane's own
  //     `useNativeScroll(!pane)` is a no-op and just READS the flag.
  // The focused MOBILE pane (`useMobileShell`) then follows the flag → native
  // document scroll + auto-hide header, like single-pane; hidden + desktop panes
  // stay on the inner shell.
  useNativeScroll(!pane)
  // Read the store flag UNCONDITIONALLY: a store-proxy read IS a subscription hook,
  // so reading it only in one ternary branch would ADD/DROP a hook when
  // `useMobileShell` flips on a focus-switch → a Rules-of-Hooks crash. Apply the
  // condition to the VALUE, never to the read.
  const appNativeScroll = AppLayout.nativeScroll
  const nativeScroll = !pane || useMobileShell ? appNativeScroll : false
  // Live MCP tool-call statuses — subscribed reactively (proxy destructure) so a
  // newly-`pending_approval` tool triggers the scroll-to-approval effect below.
  // NOTE (split-awareness, Stage-2 candidate): `toolCalls` reads the McpComposer
  // store as a process-global map (see the effect below) — not yet pane-scoped.
  const { toolCalls } = McpComposerStore

  // Split affordance: open the current conversation beside a fresh pane. On the
  // single-pane route this seeds pane 0 with the current conversation first.
  const onSplit = () => {
    if (SplitViewStore.$.panes.length === 0 && conversationId) {
      SplitViewStore.openPane({ conversationId })
    }
    SplitViewStore.openPane({ conversationId: null })
  }

  // The conversation id we've dispatched a load for (single-pane route). Used by
  // the loading gate below so the not-found / error branch never renders in the
  // PRE-LOAD frame: `loadConversation` runs in the effect below (AFTER the first
  // render), so on a fresh mount the first render still has the store's initial
  // `loading=false, conversation=null` — which would otherwise flash the
  // "Conversation not found" alert for one frame before the load even begins.
  const loadDispatchedForRef = useRef<string | null>(null)

  // Load conversation and messages on mount or when ID changes — single-pane
  // route only; in a pane `ChatPaneProvider` owns loading into the pane's own
  // store, so ConversationPane must not re-load via the (focused-pane) bridge.
  useEffect(() => {
    if (!pane && conversationId) {
      loadDispatchedForRef.current = conversationId
      chat.loadConversation(conversationId)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pane, conversationId])

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
  // One-shot latch for the pre-existing-approval seed below (it must seed once
  // THIS pane's messages have loaded, NOT on bare mount when `messages` is empty).
  const didSeedApprovalsRef = useRef(false)

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
        if (!chat.$.conversation) return
        // Each pane registers this window listener; only the FOCUSED pane opens
        // its find bar (audit #2) — otherwise Cmd-F opened it in EVERY loaded pane.
        // Single-pane (`!pane`) is always "focused".
        if (pane && pane.paneId !== SplitViewStore.$.focusedPaneId) return
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
    const cid = chat.$.conversation?.id
    if (cid && chat.$.hasMoreAfter) {
      await chat.loadMessages(cid)
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
    // Seed ONCE, the first commit where THIS pane's own conversation has loaded —
    // NOT on bare mount, where `messages` is still empty (so the old `[]`-deps
    // version seeded nothing and, on a pane REMOUNT into split view / pop-out with
    // an approval already pending in the process-global toolCalls for this pane's
    // conversation, let the scroll effect below yank the pane to that pre-existing
    // approval). Re-running on `messages` and latching via the ref means that in
    // the commit where messages first include the approval, this effect (declared
    // BEFORE the scroll effect) marks it seen first, so the scroll effect then
    // finds it already-seen and does not scroll.
    if (didSeedApprovalsRef.current) return
    if (conversation?.id !== conversationId) return
    didSeedApprovalsRef.current = true
    // Per-pane (ITEM-48): seed only THIS pane's own already-pending approvals, so a
    // leftover pending approval belonging to another pane's conversation is never
    // treated as this pane's (it's filtered out — its message isn't in `messages`).
    for (const id of pendingApprovalIdsInPane(toolCalls, messages))
      scrolledApprovalsRef.current.add(id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [conversation, conversationId, messages])

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
    // Per-pane (ITEM-48): only approvals whose carrying message is in THIS pane's
    // messages count — a pending approval in another pane's conversation must not
    // scroll this pane's list.
    for (const id of pendingApprovalIdsInPane(toolCalls, messages)) {
      if (!seen.has(id)) {
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
  }, [toolCalls, messages, conversation, conversationId, hasMoreAfter])

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
        if (!chat.$.hasMoreBefore || chat.$.loadingOlder) return
        // Capture the top-visible message as an index anchor via the virtualizer
        // so the prepend doesn't teleport the view (restored below).
        const prevFirstId = firstMessageId(chat.$.messages)
        const anchor = messageListRef.current?.captureAnchor() ?? null
        if (anchor && prevFirstId) {
          pendingAnchorRef.current = { anchor, prevFirstId }
        }
        await chat.loadOlderMessages()
        // If NO older page landed (guard early-return, empty/duplicate page, or
        // a conversation switch), the restore layout-effect never fires — clear
        // the pending anchor so it can't stick truthy and permanently suppress
        // the bottom auto-follow. `$.messages` reflects the post-`set` store.
        const pending = pendingAnchorRef.current
        if (
          pending &&
          firstMessageId(chat.$.messages) === pending.prevFirstId
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
        if (!chat.$.hasMoreAfter || chat.$.isStreaming) return
        void chat.loadNewerMessages()
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
      const found = await chat.jumpToMessage(messageId)
      if (!found || chat.$.conversation?.id !== conversation.id) return
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

  // New-chat pane adoption (ITEM-11 / TEST-37): when a pane that started with no
  // conversation (a "New chat" pane) creates one via its composer, adopt that
  // conversation into the pane's SplitView slot so it stops being a new-chat pane
  // and the whole WINDOW does not navigate (no hijack). Keyed on this pane's own
  // store's conversation becoming set.
  useEffect(() => {
    if (pane && !pane.conversationId && conversation?.id) {
      SplitViewStore.setPaneConversation(pane.paneId, conversation.id)
    }
  }, [pane, conversation?.id])

  // Loading state — also covers the PRE-LOAD frame on the single-pane route: the
  // load effect above runs after the first render, so without `loadPending` the
  // not-found branch below would flash for one frame on a fresh mount (e.g. a
  // hard reload / deep link straight to /chat/:id) before the load begins.
  const loadPending =
    !pane && !!conversationId && loadDispatchedForRef.current !== conversationId
  if ((loading || loadPending) && !conversation) {
    return <Loading />
  }

  // Empty PANE (ITEM-27): a split pane with no conversation targeted yet is the
  // second slot of a split waiting to be filled. Render the conversation PICKER
  // (searchable list of existing conversations + "Start a new chat") so the slot
  // can hold an EXISTING conversation — not only a fresh one (FB-3). The new-chat
  // path inside the picker reaches the same greeting + composer + adoption.
  // (Single-pane new chat uses the NewChatPage route instead.)
  if (pane && !conversationId && !conversation) {
    return <ConversationPickerPane paneId={pane.paneId} />
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
              conversationId && chat.loadConversation(conversationId)
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

  // Shared header controls (find toggle + split affordance + the decoupled
  // trailing slot). Reused by both the full app header (single-pane) and the
  // compact per-pane header (split).
  const headerControls = (
    <>
      {/* Find-in-conversation toggle (ITEM-1). Also openable via Cmd/Ctrl-F. */}
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
      {/* Panes affordance — the columns icon, visually distinct from the pop-out
          "new window" action that the trailing slot injects. Hidden inside the
          pop-out WINDOW (ITEM-56/FB-13): a focused single-conversation window
          shouldn't spawn a split inside itself.
            • Desktop (columns): one-click "Open in split view" (onSplit), hidden at
              MAX_PANES since another pane can't fit.
            • Small screen (FB-26): there are no tiled columns, so the button opens
              the `PaneManagerDrawer` (switch / add / close panes) instead — always
              available (even at MAX_PANES, where it's still the way to switch/close). */}
      {!isPopoutWindow &&
        (md ? (
          <Tooltip content="Panes">
            <Button
              data-testid="chat-split-btn"
              variant="ghost"
              size="icon"
              icon={<Columns2 />}
              aria-label="Open panes"
              aria-haspopup="dialog"
              onClick={() => SplitViewStore.setPaneManagerOpen(true)}
            />
          </Tooltip>
        ) : (
          splitViewPanes.length < SPLIT_LIMITS.MAX_PANES && (
            <Tooltip content="Open in split view">
              <Button
                data-testid="chat-split-btn"
                variant="ghost"
                size="icon"
                icon={<Columns2 />}
                aria-label="Open in split view"
                onClick={onSplit}
              />
            </Tooltip>
          )
        ))}
      {/* Decoupled chip injection point — other modules register header
          decorations into `chatConversationHeaderTrailing` without chat
          compiling against them. */}
      <ConversationHeaderTrailingSlot />
    </>
  )

  return (
    <div className={cn('flex flex-col', nativeScroll ? 'min-h-dvh' : 'h-full')}>
      {/* Header — full width app header on the single-pane route; an in-column
          header inside a split pane. On DESKTOP the pane header is compact (reorder
          grip + close-pane ✕ + drag-to-split drop target). On SMALL SCREENS
          (`md`, FB-26) the split can't tile columns, so the pane reads as a normal
          single-pane conversation: normal left inset (clears the sidebar toggle for
          EVERY focused pane, not just the leftmost — fixing the "2nd pane loses the
          inset" bug), NO grip, NO per-pane ✕, and NO drag chrome (pane management
          moves to the `PaneManagerDrawer`). One element, adapted by `md`, so the
          `chat-pane-header` testid stays unique. The FOCUSED mobile pane
          (`useMobileShell`) instead falls through to the `HeaderBarContainer` branch
          below (same as single-pane) so it INHERITS the real auto-hide chrome —
          sticky `top:5` (dodges iOS's under-notch latch), the `bg-card` backdrop that
          fills the safe-area/notch gap, and the relative-wipe-on-scroll-down — rather
          than a re-derived approximation (FB-28). The store-proxy flag is read
          unconditionally above, so this branch flip does not change the hook tree. */}
      {pane && !useMobileShell ? (
        <div
          className={cn(
            'flex h-[50px] shrink-0 items-center justify-between gap-2 border-b',
            !md && paneDropActive && 'bg-primary/10 ring-2 ring-primary ring-inset',
          )}
          // Match HeaderBarContainer: 50px tall. Small screen → always reserve the
          // toggle inset (the focused pane is the ONLY visible one). Desktop → only
          // the leftmost pane reserves it (else a plain 12px); FB-18.
          style={{ paddingLeft: md || isLeftmostPane ? headerLeftInset : 12, paddingRight: 12 }}
          data-testid="chat-pane-header"
          onDragOver={md ? undefined : onPaneAreaDragOver}
          onDragLeave={md ? undefined : onPaneAreaDragLeave}
          onDrop={md ? undefined : onPaneAreaDrop}
        >
          <div className="flex min-w-0 items-center gap-2">
            {/* Reorder handle (ITEM-31): drag this pane's header onto another
                pane to reorder. A span (not a button) so it doesn't steal the
                title's focus; keyboard reorder is out of scope for the handle.
                Tear-off (ITEM-58): releasing the same drag PAST the window edge
                pops this pane's conversation into its own desktop window and
                MOVES it out (no-op on web / in-window release). Desktop-only:
                drag is meaningless on touch/narrow (FB-26). */}
            {!md && (
              <span
                draggable
                onDragStart={e => setPaneDragData(e.dataTransfer, pane.paneId)}
                onDragEnd={e => {
                  if (pane.conversationId)
                    tearOff(e, {
                      conversationId: pane.conversationId,
                      paneId: pane.paneId,
                      title: conversation?.title ?? undefined,
                    })
                }}
                data-testid="chat-pane-grip"
                aria-label="Drag to reorder pane"
                className="shrink-0 cursor-grab text-muted-foreground active:cursor-grabbing"
              >
                <GripVertical className="size-4" />
              </span>
            )}
            <TitleEditor />
          </div>
          <div className="flex items-center gap-1">
            {headerControls}
            {/* Per-pane close ✕ — desktop only; on small screens a pane is closed
                from the PaneManagerDrawer's ✕ (FB-26). */}
            {!md && (
              <Tooltip content="Close pane">
                <Button
                  data-testid="chat-pane-close"
                  variant="ghost"
                  size="icon"
                  icon={<X />}
                  aria-label="Close pane"
                  onClick={() => closePane(pane.paneId)}
                />
              </Tooltip>
            )}
          </div>
        </div>
      ) : (
        <HeaderBarContainer>
          <div className="h-full flex items-center justify-between w-full gap-2">
            <div className="flex items-center min-w-0 gap-2">
              <TitleEditor />
            </div>
            <div className="flex items-center gap-1">{headerControls}</div>
          </div>
        </HeaderBarContainer>
      )}

      {/* Error banner */}
      {error && (
        <div className="w-full max-w-4xl mx-auto px-4 pt-4">
          <Alert data-testid="chat-conversation-error-alert" tone="error" title={error} onClose={chat.clearError} closeLabel="Close" />
        </div>
      )}

      {/* Main area: chat column + right panel */}
      <div ref={mainAreaRef} className={cn('relative flex flex-1 min-h-0', nativeScroll ? '' : 'overflow-hidden')}>
        {/* Chat column. `relative` anchors the floating find bar (ITEM-1) and
            jump-to-latest button (ITEM-2). */}
        <div
          // Testid only in single-pane (kept unique — split panes have `pane`); a
          // plain data attr marks the per-pane drop column for split e2e targeting.
          data-testid={pane ? undefined : 'chat-single-drop-column'}
          data-pane-drop-column="true"
          className={cn('relative flex flex-col flex-1 min-w-0', nativeScroll ? '' : 'overflow-hidden')}
          // Drag-to-split / edge-drop is desktop-only (FB-26): meaningless on
          // touch/narrow, where pane management lives in the PaneManagerDrawer.
          onDragOver={md ? undefined : onPaneAreaDragOver}
          onDragLeave={md ? undefined : onPaneAreaDragLeave}
          onDrop={md ? undefined : onPaneAreaDrop}
        >
          {/* Edge-directional drop hint (ITEM-57 single-pane / ITEM-70 split): while
              a conversation is dragged over the column, show the three target thirds
              and highlight the one under the pointer. Non-interactive overlay — the
              drag events land on the column beneath it. Labels reflect the action:
              single-pane opens a split by side; a split pane inserts a new pane
              before/after (or replaces at the MAX_PANES cap). */}
          {!md && dropZone && (
            <div className="pointer-events-none absolute inset-0 z-40 flex" aria-hidden="true">
              {(['left', 'center', 'right'] as const).map(z => {
                // Snapshot read (`.$`) — NOT the reactive proxy — so this is a plain
                // value read, not a hook call inside a loop/conditional (Rules of Hooks).
                const atCap =
                  !!pane && SplitViewStore.$.panes.length >= SPLIT_LIMITS.MAX_PANES
                const label =
                  z === 'center'
                    ? 'Replace'
                    : pane
                      ? atCap
                        ? 'Replace'
                        : z === 'left'
                          ? 'Insert left'
                          : 'Insert right'
                      : z === 'left'
                        ? 'Open on left'
                        : 'Open on right'
                return (
                  <div
                    key={z}
                    className={cn(
                      'flex flex-1 items-center justify-center border border-dashed border-primary/30 text-sm font-medium text-primary transition-colors',
                      dropZone === z
                        ? 'bg-primary/10 ring-2 ring-inset ring-primary'
                        : 'opacity-40',
                    )}
                  >
                    {dropZone === z && (
                      <span className="rounded-md bg-card/80 px-2 py-1 shadow-sm">{label}</span>
                    )}
                  </div>
                )
              })}
            </div>
          )}
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

        {/* Right sidebar panel. Inside a split pane it renders as an in-pane
            slide-over anchored to this pane's main area (ITEM-18); on the full
            page it's the inline side panel / narrow Drawer as before. */}
        <ChatRightPanel narrow={rightPanelNarrow} inPane={!!pane} />
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
  const { slots } = ModuleSystem
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
