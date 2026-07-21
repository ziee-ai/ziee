/**
 * TEST-1..5 — ChatHistory sidebar "recent chats" infinite-scroll paging.
 *
 * Asserts the DEDICATED `recent*` paging cursor (decoupled from the /chats
 * history query):
 *   - TEST-1  loadRecentConversations(1) populates the first page + flags
 *   - TEST-2  loadMoreRecent() appends the next page, dedups a boundary id,
 *             no-ops while a load is in flight
 *   - TEST-3  paging to the last page sets recentHasMore=false + a further
 *             loadMoreRecent() is a no-op
 *   - TEST-4  the history query (loadConversations) does NOT mutate the sidebar
 *             list (decoupled)
 *   - TEST-5  conversation.created prepends WITHOUT the old 20-cap + bumps
 *             recentTotal; a sync delete removes the row + decrements recentTotal
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ConversationResponse } from '@/api-client/types'

const apiMock = vi.hoisted(() => ({
  Conversation: {
    list: vi.fn(),
    delete: vi.fn(() => Promise.resolve({})),
    update: vi.fn(() => Promise.resolve({})),
  },
}))

const perm = vi.hoisted(() => ({ allow: true }))

const bus = vi.hoisted(() => {
  const map = new Map<string, Set<(p?: unknown) => void>>()
  return {
    on: (event: string, handler: (p?: unknown) => void) => {
      let s = map.get(event)
      if (!s) {
        s = new Set()
        map.set(event, s)
      }
      s.add(handler)
      return () => s?.delete(handler)
    },
    removeGroupListeners: () => {},
    emit: (event: string, payload?: unknown) =>
      map.get(event)?.forEach(fn => fn(payload)),
    clear: () => map.clear(),
  }
})

vi.mock('@/api-client', () => ({ ApiClient: apiMock }))
vi.mock('@/core/permissions', () => ({
  hasPermissionNow: () => perm.allow,
  Permissions: {},
}))
vi.mock('@ziee/framework/stores', () => ({
  Stores: { EventBus: { emit: vi.fn(() => Promise.resolve()) } },
  createStoreProxy: () => ({}),
}))
vi.mock('@ziee/framework/events', () => ({
  useEventBusStore: {
    getState: () => ({
      on: bus.on,
      removeGroupListeners: bus.removeGroupListeners,
    }),
  },
}))

import { useChatHistoryStore } from './chatHistory'

const store = () => useChatHistoryStore.getState()

function convo(over: Partial<ConversationResponse> = {}): ConversationResponse {
  return {
    id: 'c1',
    title: 'Conversation',
    user_id: 'u1',
    created_at: '2026-07-11T00:00:00Z',
    updated_at: '2026-07-11T00:00:00Z',
    message_count: 0,
    ...over,
  }
}

// Page factory: a page of `count` conversations id-prefixed `p<page>-<i>`.
function page(pageNo: number, count: number, total: number) {
  return {
    conversations: Array.from({ length: count }, (_, i) =>
      convo({ id: `p${pageNo}-${i}`, title: `chat ${pageNo}-${i}` }),
    ),
    total,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  perm.allow = true
  bus.clear()
  useChatHistoryStore.setState({
    conversations: [],
    recentConversations: [],
    page: 1,
    limit: 20,
    total: 0,
    hasMore: false,
    recentPage: 1,
    recentTotal: 0,
    recentHasMore: false,
    recentLoading: false,
    recentLoadingMore: false,
    recentInitialized: false,
    recentError: null,
    recentLoadSeq: 0,
    searchQuery: '',
    sort: 'recent',
    selectedIds: new Set(),
    loading: false,
    loadingMore: false,
    reloadQueued: false,
  })
})

describe('ChatHistory recent paging (TEST-1..5)', () => {
  it('TEST-1: loadRecentConversations(1) loads the first page unfiltered', async () => {
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 20, 45))

    await store().loadRecentConversations(1)

    expect(apiMock.Conversation.list).toHaveBeenCalledWith({ page: 1, limit: 20 })
    const s = store()
    expect(s.recentConversations).toHaveLength(20)
    expect(s.recentTotal).toBe(45)
    expect(s.recentHasMore).toBe(true)
    expect(s.recentInitialized).toBe(true)
    expect(s.recentPage).toBe(1)
    expect(s.recentLoading).toBe(false)
  })

  it('TEST-1b: self-gates on ConversationsRead (no perm → no request)', async () => {
    perm.allow = false
    await store().loadRecentConversations(1)
    expect(apiMock.Conversation.list).not.toHaveBeenCalled()
    expect(store().recentInitialized).toBe(false)
  })

  it('TEST-2: loadMoreRecent() appends the next page and dedups a boundary id', async () => {
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 20, 45))
    await store().loadRecentConversations(1)

    // Page 2 includes a duplicate of a page-1 row (`p1-19`) at its head.
    apiMock.Conversation.list.mockResolvedValueOnce({
      conversations: [
        convo({ id: 'p1-19', title: 'dup' }),
        ...Array.from({ length: 20 }, (_, i) => convo({ id: `p2-${i}` })),
      ],
      total: 45,
    })

    await store().loadMoreRecent()

    const s = store()
    // 20 + 20 new (the duplicate is dropped, not 21).
    expect(s.recentConversations).toHaveLength(40)
    expect(s.recentConversations.filter(c => c.id === 'p1-19')).toHaveLength(1)
    expect(s.recentPage).toBe(2)
    expect(s.recentHasMore).toBe(true)
    expect(apiMock.Conversation.list).toHaveBeenLastCalledWith({ page: 2, limit: 20 })
  })

  it('TEST-2b: loadMoreRecent() is a no-op while a load is in flight', async () => {
    useChatHistoryStore.setState({
      recentConversations: [convo({ id: 'x' })],
      recentHasMore: true,
      recentLoadingMore: true,
    })
    await store().loadMoreRecent()
    expect(apiMock.Conversation.list).not.toHaveBeenCalled()
  })

  it('TEST-3: reaching the last page clears recentHasMore and stops', async () => {
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 20, 40))
    await store().loadRecentConversations(1)
    apiMock.Conversation.list.mockResolvedValueOnce(page(2, 20, 40))
    await store().loadMoreRecent()

    expect(store().recentConversations).toHaveLength(40)
    expect(store().recentHasMore).toBe(false)

    // A further loadMoreRecent must NOT fetch (end-of-list).
    apiMock.Conversation.list.mockClear()
    await store().loadMoreRecent()
    expect(apiMock.Conversation.list).not.toHaveBeenCalled()
  })

  it('TEST-4: the history query does NOT mutate the sidebar recent list', async () => {
    const seeded = [convo({ id: 'r0' }), convo({ id: 'r1' }), convo({ id: 'r2' })]
    useChatHistoryStore.setState({
      recentConversations: seeded,
      recentInitialized: true,
      recentTotal: 3,
    })
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 20, 99))

    await store().loadConversations(1)

    expect(store().conversations).toHaveLength(20)
    expect(store().total).toBe(99)
    // Decoupled: the accumulated sidebar list survives a /chats reload untouched.
    expect(store().recentConversations).toEqual(seeded)
    expect(store().recentTotal).toBe(3)
  })

  it('TEST-5: conversation.created prepends without a 20-cap; sync delete prunes + counts', () => {
    store().__init__.__store__() // wire init event handlers

    const forty = Array.from({ length: 40 }, (_, i) => convo({ id: `r${i}` }))
    useChatHistoryStore.setState({
      recentConversations: forty,
      recentInitialized: true,
      recentTotal: 40,
    })

    bus.emit('conversation.created', {
      data: { conversation: convo({ id: 'brand-new', title: 'New chat' }) },
    })

    let s = store()
    expect(s.recentConversations).toHaveLength(41) // NOT truncated to 20
    expect(s.recentConversations[0].id).toBe('brand-new')
    expect(s.recentTotal).toBe(41)
    // Cursor re-anchored to the grown length so accumulated local creates don't
    // strand older pages (floor(41/20) = 2).
    expect(s.recentPage).toBe(2)

    // A cross-device delete of a loaded row prunes it and decrements the counter.
    bus.emit('sync:conversation', { data: { action: 'delete', id: 'r5' } })
    s = store()
    expect(s.recentConversations.some(c => c.id === 'r5')).toBe(false)
    expect(s.recentConversations).toHaveLength(40)
    expect(s.recentTotal).toBe(40)
  })

  it('TEST-3b: a short server page ends paging even if recentTotal is drifted high', async () => {
    // total claims 45 but the server actually returns a short last page (proof
    // the end-detection is anchored on the page size, not the length<total math).
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 20, 45))
    await store().loadRecentConversations(1)
    // Page 2 comes back SHORT (5 < limit 20) — no more data — while total still 45.
    apiMock.Conversation.list.mockResolvedValueOnce({
      conversations: Array.from({ length: 5 }, (_, i) => convo({ id: `p2-${i}` })),
      total: 45,
    })
    await store().loadMoreRecent()

    expect(store().recentConversations).toHaveLength(25)
    // Even though 25 < 45, the short page proves the end → no runaway loadMore.
    expect(store().recentHasMore).toBe(false)
    apiMock.Conversation.list.mockClear()
    await store().loadMoreRecent()
    expect(apiMock.Conversation.list).not.toHaveBeenCalled()
  })

  it('TEST-3c: a next page that adds nothing new (all dups) also stops paging', async () => {
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 20, 45))
    await store().loadRecentConversations(1)
    // Page 2 is a full page but every id is already loaded (pure overlap).
    apiMock.Conversation.list.mockResolvedValueOnce({
      conversations: Array.from({ length: 20 }, (_, i) => convo({ id: `p1-${i}` })),
      total: 45,
    })
    await store().loadMoreRecent()
    expect(store().recentConversations).toHaveLength(20) // nothing appended
    expect(store().recentHasMore).toBe(false) // no-progress → stop, no runaway
  })

  it('TEST-3d: a first-load failure sets recentError and does not wedge on the spinner', async () => {
    apiMock.Conversation.list.mockRejectedValueOnce(new Error('network'))
    await store().loadRecentConversations(1)
    expect(store().recentError).toBe('Failed to load conversations')
    expect(store().recentLoading).toBe(false)
    // A retry after recovery clears the error and loads.
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 10, 10))
    await store().loadRecentConversations(1)
    expect(store().recentError).toBeNull()
    expect(store().recentConversations).toHaveLength(10)
  })

  it('TEST-5b: syncRecentFront() merge-prepends without resetting loaded pages', async () => {
    // A 40-row accumulated sidebar (2 pages scrolled in).
    const forty = Array.from({ length: 40 }, (_, i) => convo({ id: `r${i}` }))
    useChatHistoryStore.setState({
      recentConversations: forty,
      recentInitialized: true,
      recentTotal: 45,
      recentHasMore: true,
      recentPage: 2,
    })

    // Page 1 refetch: a brand-new row `n0` on top of the existing page-1 rows.
    apiMock.Conversation.list.mockResolvedValueOnce({
      conversations: [
        convo({ id: 'n0', title: 'from other device' }),
        ...Array.from({ length: 19 }, (_, i) => convo({ id: `r${i}` })),
      ],
      total: 46,
    })

    await store().syncRecentFront()

    const s = store()
    // The new row is prepended; the accumulated older pages are NOT dropped.
    expect(s.recentConversations[0].id).toBe('n0')
    expect(s.recentConversations).toHaveLength(41)
    expect(s.recentConversations.some(c => c.id === 'r39')).toBe(true) // page-2 row survives
    expect(s.recentTotal).toBe(46)
  })

  it('TEST-14: deleting the last loaded rows while more exist refills page 1', async () => {
    // One loaded row, but the server has many more (recentHasMore=true).
    useChatHistoryStore.setState({
      recentConversations: [convo({ id: 'only' })],
      recentInitialized: true,
      recentTotal: 45,
      recentHasMore: true,
      recentPage: 1,
    })
    apiMock.Conversation.delete.mockResolvedValueOnce({})
    // The refill's page-1 reload returns the next batch.
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 20, 44))

    await store().deleteConversation('only')

    // The sidebar did NOT strand on empty — it refilled from the server.
    expect(apiMock.Conversation.list).toHaveBeenCalledWith({ page: 1, limit: 20 })
    expect(store().recentConversations).toHaveLength(20)
    expect(store().recentHasMore).toBe(true)
  })

  it('TEST-14b: deleting the last loaded rows when NONE remain does NOT refetch', async () => {
    useChatHistoryStore.setState({
      recentConversations: [convo({ id: 'only' })],
      recentInitialized: true,
      recentTotal: 1,
      recentHasMore: false, // fully loaded — nothing more server-side
      recentPage: 0,
    })
    apiMock.Conversation.delete.mockResolvedValueOnce({})
    await store().deleteConversation('only')
    expect(apiMock.Conversation.list).not.toHaveBeenCalled()
    expect(store().recentConversations).toHaveLength(0)
  })

  it('TEST-14c: an in-flight loadMore whose list is reset mid-flight is discarded (epoch)', async () => {
    // Page 1 loaded, more server-side.
    useChatHistoryStore.setState({
      recentConversations: Array.from({ length: 20 }, (_, i) => convo({ id: `r${i}` })),
      recentInitialized: true,
      recentTotal: 45,
      recentHasMore: true,
      recentPage: 1,
    })

    // Kick off a page-2 loadMore whose response we control (deferred).
    let resolvePage2: (v: unknown) => void = () => {}
    const page2 = new Promise(res => {
      resolvePage2 = res
    })
    apiMock.Conversation.list.mockReturnValueOnce(page2 as any)
    const morePromise = store().loadMoreRecent() // do NOT await — in flight now
    expect(store().recentLoadingMore).toBe(true)

    // While it's in flight, a delete drains the list to empty → refill bumps the
    // epoch, clears the flags, and reloads page 1 (fresh rows).
    useChatHistoryStore.setState({ recentConversations: [] })
    apiMock.Conversation.list.mockResolvedValueOnce({
      conversations: Array.from({ length: 20 }, (_, i) => convo({ id: `fresh${i}` })),
      total: 44,
    })
    await store().refillRecentIfEmptied()
    expect(store().recentConversations[0].id).toBe('fresh0')

    // NOW the stale page-2 resolves — it must be DISCARDED (epoch changed), not
    // appended onto the freshly-reloaded list.
    resolvePage2({
      conversations: Array.from({ length: 20 }, (_, i) => convo({ id: `stale${i}` })),
      total: 45,
    })
    await morePromise

    const ids = store().recentConversations.map(c => c.id)
    expect(ids).toContain('fresh0')
    expect(ids.some(id => id.startsWith('stale'))).toBe(false) // stale page dropped
  })

  it('TEST-14d: a delete concurrent with an in-flight loadMore re-anchors recentPage (no skip)', async () => {
    useChatHistoryStore.setState({
      recentConversations: Array.from({ length: 40 }, (_, i) => convo({ id: `r${i}` })),
      recentInitialized: true,
      recentTotal: 100,
      recentHasMore: true,
      recentPage: 2,
    })

    // loadMore(page 3) in flight (deferred response).
    let resolveP3: (v: unknown) => void = () => {}
    const p3 = new Promise(res => {
      resolveP3 = res
    })
    apiMock.Conversation.list.mockReturnValueOnce(p3 as any)
    const morePromise = store().loadMoreRecent()
    expect(store().recentLoadingMore).toBe(true)

    // A delete of a loaded row (list NOT drained to empty) runs mid-flight.
    apiMock.Conversation.delete.mockResolvedValueOnce({})
    await store().deleteConversation('r5')
    expect(store().recentConversations).toHaveLength(39)
    expect(store().recentPage).toBe(1) // floor(39/20)

    // The in-flight page 3 resolves → appends, re-anchoring recentPage to the
    // loaded length, NOT the stale targetPage(3) — so the next loadMore overlaps
    // the tail (dedup) instead of skipping a server row.
    resolveP3({
      conversations: Array.from({ length: 20 }, (_, i) => convo({ id: `n${i}` })),
      total: 99,
    })
    await morePromise
    expect(store().recentConversations).toHaveLength(59)
    expect(store().recentPage).toBe(2) // floor(59/20) = 2, NOT the stale 3
  })

  it('TEST-5c: syncRecentFront re-anchors recentPage so paging keeps reaching older rows', async () => {
    // One page loaded (recentPage=1), then a big cross-device burst prepends a
    // full page of new rows.
    apiMock.Conversation.list.mockResolvedValueOnce(page(1, 20, 45))
    await store().loadRecentConversations(1)

    apiMock.Conversation.list.mockResolvedValueOnce({
      conversations: Array.from({ length: 20 }, (_, i) => convo({ id: `n${i}` })),
      total: 65,
    })
    await store().syncRecentFront()

    // Cursor re-anchored to the grown length (40/20 = 2), NOT left at 1.
    expect(store().recentConversations).toHaveLength(40)
    expect(store().recentPage).toBe(2)
    expect(store().recentHasMore).toBe(true)

    // The next loadMore fetches page 3 (older, unseen) and PROGRESSES — it does
    // not dead-end on an all-overlap page.
    apiMock.Conversation.list.mockResolvedValueOnce({
      conversations: Array.from({ length: 20 }, (_, i) => convo({ id: `old${i}` })),
      total: 65,
    })
    await store().loadMoreRecent()
    expect(apiMock.Conversation.list).toHaveBeenLastCalledWith({ page: 3, limit: 20 })
    expect(store().recentConversations).toHaveLength(60)
    expect(store().recentHasMore).toBe(true)
  })
})
