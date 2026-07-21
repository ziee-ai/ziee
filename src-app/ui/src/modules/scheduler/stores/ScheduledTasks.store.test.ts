import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@/core/permissions', () => ({ hasPermissionNow: () => true }))

const listRuns = vi.fn()
const continueSeries = vi.fn()
vi.mock('@/api-client', () => ({
  ApiClient: {
    ScheduledTask: {
      list: vi.fn().mockResolvedValue([]),
      listRuns: (...a: unknown[]) => listRuns(...a),
      continueSeries: (...a: unknown[]) => continueSeries(...a),
    },
  },
}))

import { useScheduledTasksStore } from './scheduledTasks/index'

const s = () => useScheduledTasksStore.getState()

// TEST-53 / TEST-55 (ITEM-41/46/47): the store pages run history + maps series limit.
describe('ScheduledTasks store — runs paging + series', () => {
  beforeEach(() => {
    listRuns.mockReset()
    continueSeries.mockReset()
  })

  it('loadRuns stores the paged slice + total; a page change refetches that page', async () => {
    listRuns.mockResolvedValueOnce({
      runs: [{ id: 'r1' }],
      total: 25,
      page: 1,
      per_page: 10,
    })
    await s().loadRuns('t1', 1)
    expect(listRuns).toHaveBeenCalledWith({ id: 't1', page: 1, per_page: 10 })
    expect(s().runsByTask['t1']).toHaveLength(1)
    expect(s().runsMetaByTask['t1']).toEqual({ total: 25, page: 1, perPage: 10 })

    listRuns.mockResolvedValueOnce({
      runs: [{ id: 'r2' }],
      total: 25,
      page: 2,
      per_page: 10,
    })
    await s().loadRuns('t1', 2)
    expect(listRuns).toHaveBeenLastCalledWith({ id: 't1', page: 2, per_page: 10 })
    expect(s().runsMetaByTask['t1'].page).toBe(2)
  })

  it('snaps an out-of-range page (empty page, total>0) back to page 1', async () => {
    // First call: page 3 comes back empty but total=5 (history shrank) → the store
    // must refetch page 1 rather than strand the user.
    listRuns
      .mockResolvedValueOnce({ runs: [], total: 5, page: 3, per_page: 10 })
      .mockResolvedValueOnce({ runs: [{ id: 'r1' }], total: 5, page: 1, per_page: 10 })
    await s().loadRuns('t1', 3)
    expect(listRuns).toHaveBeenCalledTimes(2)
    expect(listRuns).toHaveBeenLastCalledWith({ id: 't1', page: 1, per_page: 10 })
    expect(s().runsByTask['t1']).toHaveLength(1)
    expect(s().runsMetaByTask['t1'].page).toBe(1)
  })

  it('continueSeries maps the selection to the limit query param', async () => {
    continueSeries.mockResolvedValueOnce({ conversation_id: 'c9' })
    const id = await s().continueSeries('t1', 10)
    expect(continueSeries).toHaveBeenCalledWith({ id: 't1', limit: 10 })
    expect(id).toBe('c9')
  })
})
