import { beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * TEST-14 (ITEM-9) — the sub-agent transcript drill-in store action. Mocks ONLY
 * the ApiClient boundary (`GET /api/subagent-runs/{child_id}`) and asserts the
 * real `childDetailsById` state transitions: `loading` → `loaded` (filtered
 * agent-activity transcript + run status) on success, and → `{status:'error'}`
 * on a 404 (a pruned parent/child) so the card degrades to status-only.
 */

const getSubAgentRun = vi.fn()
vi.mock('@/api-client', () => ({
  ApiClient: {
    SubAgentRuns: {
      get: (...a: unknown[]) => getSubAgentRun(...a),
    },
  },
}))

import { createSubAgentActivityStore } from './SubAgentActivity.store'

describe('SubAgentActivityStore — loadChildTranscript (ITEM-9)', () => {
  beforeEach(() => {
    getSubAgentRun.mockReset()
  })

  it('populates childDetailsById with the filtered transcript + run status on success', async () => {
    const store = createSubAgentActivityStore()
    getSubAgentRun.mockResolvedValueOnce({
      id: 'child-1',
      status: 'completed',
      // A mixed ProgressKind[] — only the agent_activity entries survive the filter.
      activity: [
        { type: 'agent_activity', seq: 0, kind: 'thinking', title: 'Thinking', status: 'ok' },
        { type: 'log', line: 'noise' },
        { type: 'agent_activity', seq: 1, kind: 'tool', title: 'Searched', status: 'ok' },
      ],
    })

    await store.loadChildTranscript('child-1')

    expect(getSubAgentRun).toHaveBeenCalledWith({ child_id: 'child-1' })
    const detail = store.$.childDetailsById['child-1']
    expect(detail?.status).toBe('loaded')
    if (detail?.status !== 'loaded') throw new Error('expected loaded')
    expect(detail.runStatus).toBe('completed')
    // The `log` entry is dropped; the two agent_activity entries survive in order.
    expect(detail.activity).toHaveLength(2)
    expect(detail.activity.every(e => e.type === 'agent_activity')).toBe(true)
    expect(detail.activity.map(e => e.seq)).toEqual([0, 1])
  })

  it('tolerates a run with no activity — loaded with an empty transcript', async () => {
    const store = createSubAgentActivityStore()
    getSubAgentRun.mockResolvedValueOnce({ id: 'child-2', status: 'completed' })

    await store.loadChildTranscript('child-2')

    const detail = store.$.childDetailsById['child-2']
    expect(detail).toEqual({ status: 'loaded', activity: [], runStatus: 'completed' })
  })

  it('resolves to {status:"error"} on a 404 (pruned child) without throwing', async () => {
    const store = createSubAgentActivityStore()
    getSubAgentRun.mockRejectedValueOnce(new Error('Not Found'))

    // MUST NOT throw — a pruned parent/child degrades gracefully.
    await expect(store.loadChildTranscript('gone')).resolves.toBeUndefined()

    expect(store.$.childDetailsById['gone']).toEqual({ status: 'error' })
  })

  it('is idempotent for a cached loaded child (re-expand never refetches)', async () => {
    const store = createSubAgentActivityStore()
    getSubAgentRun.mockResolvedValueOnce({ id: 'child-3', status: 'completed', activity: [] })

    await store.loadChildTranscript('child-3')
    await store.loadChildTranscript('child-3')

    expect(getSubAgentRun).toHaveBeenCalledTimes(1)
  })

  it('retries a previously-errored child on the next expand', async () => {
    const store = createSubAgentActivityStore()
    getSubAgentRun.mockRejectedValueOnce(new Error('boom'))
    await store.loadChildTranscript('child-4')
    expect(store.$.childDetailsById['child-4']).toEqual({ status: 'error' })

    getSubAgentRun.mockResolvedValueOnce({ id: 'child-4', status: 'completed', activity: [] })
    await store.loadChildTranscript('child-4')

    expect(getSubAgentRun).toHaveBeenCalledTimes(2)
    expect(store.$.childDetailsById['child-4']?.status).toBe('loaded')
  })
})
