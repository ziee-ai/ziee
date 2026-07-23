/**
 * TEST-19 — the agent-activity merge helpers backing the WorkflowRun store's
 * ACTIVITY TIMELINE, plus the store reducer that routes agent_activity tracks
 * into the timeline while leaving ordinary progress tracks untouched.
 *
 * The pure helpers (`mergeAgentActivity` / `mergeAgentActivityBatch`) were
 * un-exported; they are now exported (behaviour-preserving) so the ordering /
 * dedupe / cap invariants can be pinned directly. The store itself is a global
 * `defineStore`, so it's driven headless (getState) with the SSE seam mocked —
 * the same pattern the voice download-progress store test uses.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { AgentActivityEntry } from '../components/run/activityDescriptors'

// ── mocked seams (so importing the store module doesn't touch the network) ──

const apiMock = vi.hoisted(() => ({
  Workflow: {
    cancelRun: vi.fn(() => Promise.resolve({})),
    submitElicit: vi.fn(() => Promise.resolve({})),
  },
}))

/** Captures the handlers passed to `subscribeRunProgress` so a test can feed
 *  frames straight into the real store reducers. */
const sse = vi.hoisted(() => ({ handlers: null as any, closed: false }))

vi.mock('@/api-client', () => ({ ApiClient: apiMock }))
vi.mock('@ziee/framework/stores', () => ({
  Stores: {},
  createStoreProxy: () => ({}),
}))
vi.mock('@ziee/framework/events', () => ({
  useEventBusStore: {
    getState: () => ({ on: () => () => {}, removeGroupListeners: () => {} }),
  },
}))
vi.mock('@/modules/workflow/sse/runProgressClient', () => ({
  subscribeRunProgress: (_runId: string, handlers: any) => {
    sse.handlers = handlers
    sse.closed = false
    return {
      close: () => {
        sse.closed = true
      },
    }
  },
}))

import {
  AGENT_ACTIVITY_MAX_ENTRIES,
  mergeAgentActivity,
  mergeAgentActivityBatch,
} from './workflowRun/agentActivity'
import { useWorkflowRunStore } from './workflowRun'

/** Build a well-formed agent_activity entry. */
const act = (seq: number, over: Partial<AgentActivityEntry> = {}): AgentActivityEntry =>
  ({
    type: 'agent_activity',
    title: `entry ${seq}`,
    kind: 'tool_call',
    seq,
    status: 'running',
    tool: 'web_search',
    ...over,
  }) as AgentActivityEntry

describe('mergeAgentActivity (single-entry merge)', () => {
  it('appends a strictly-higher-seq entry to the tail', () => {
    const list: AgentActivityEntry[] = [act(1), act(2)]
    mergeAgentActivity(list, act(3))
    expect(list.map(e => e.seq)).toEqual([1, 2, 3])
  })

  it('replaces the tail in place on an equal-seq status upgrade (no duplicate)', () => {
    const list: AgentActivityEntry[] = [act(1), act(2, { status: 'running' })]
    mergeAgentActivity(list, act(2, { status: 'ok', title: 'done' }))
    expect(list.map(e => e.seq)).toEqual([1, 2])
    expect(list[1].status).toBe('ok')
    expect(list[1].title).toBe('done')
  })

  it('dedupes an out-of-order re-send of an existing seq (replace, keep order)', () => {
    const list: AgentActivityEntry[] = [act(1), act(2), act(3)]
    // seq 2 arrives again after 3 (out of order) with an upgraded status.
    mergeAgentActivity(list, act(2, { status: 'ok' }))
    expect(list.map(e => e.seq)).toEqual([1, 2, 3])
    expect(list.find(e => e.seq === 2)?.status).toBe('ok')
  })

  it('splice-inserts a genuinely out-of-order NEW seq into ascending position', () => {
    const list: AgentActivityEntry[] = [act(1), act(4)]
    mergeAgentActivity(list, act(2))
    expect(list.map(e => e.seq)).toEqual([1, 2, 4])
  })

  it('caps at AGENT_ACTIVITY_MAX_ENTRIES, dropping the lowest-seq head', () => {
    const list: AgentActivityEntry[] = []
    for (let s = 1; s <= AGENT_ACTIVITY_MAX_ENTRIES; s++) mergeAgentActivity(list, act(s))
    expect(list.length).toBe(AGENT_ACTIVITY_MAX_ENTRIES)
    // One more over the cap drops the oldest (seq 1) and keeps the newest.
    mergeAgentActivity(list, act(AGENT_ACTIVITY_MAX_ENTRIES + 1))
    expect(list.length).toBe(AGENT_ACTIVITY_MAX_ENTRIES)
    expect(list[0].seq).toBe(2)
    expect(list[list.length - 1].seq).toBe(AGENT_ACTIVITY_MAX_ENTRIES + 1)
  })
})

describe('mergeAgentActivityBatch (bulk snapshot merge)', () => {
  it('merges a persisted array in one pass: dedupes existing, appends new, keeps sorted', () => {
    const list: AgentActivityEntry[] = [act(1), act(2, { status: 'running' })]
    mergeAgentActivityBatch(list, [act(2, { status: 'ok' }), act(3), act(4)])
    expect(list.map(e => e.seq)).toEqual([1, 2, 3, 4])
    expect(list.find(e => e.seq === 2)?.status).toBe('ok')
  })

  it('re-sorts out-of-order incoming rows by seq', () => {
    const list: AgentActivityEntry[] = []
    mergeAgentActivityBatch(list, [act(3), act(1), act(2)])
    expect(list.map(e => e.seq)).toEqual([1, 2, 3])
  })

  it('an empty incoming array is a no-op', () => {
    const list: AgentActivityEntry[] = [act(1)]
    mergeAgentActivityBatch(list, [])
    expect(list.map(e => e.seq)).toEqual([1])
  })

  it('caps the merged result at AGENT_ACTIVITY_MAX_ENTRIES', () => {
    const incoming: AgentActivityEntry[] = []
    for (let s = 1; s <= AGENT_ACTIVITY_MAX_ENTRIES + 50; s++) incoming.push(act(s))
    const list: AgentActivityEntry[] = []
    mergeAgentActivityBatch(list, incoming)
    expect(list.length).toBe(AGENT_ACTIVITY_MAX_ENTRIES)
    expect(list[0].seq).toBe(51) // lowest 50 dropped
  })
})

describe('stepProgress reducer routing (store-level)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    sse.handlers = null
    useWorkflowRunStore.setState({ runs: {}, cancelling: {}, submittingElicit: {} })
  })

  it('routes agent_activity into the timeline and leaves a NON-agent track untouched', () => {
    const runId = 'run-1'
    useWorkflowRunStore.getState().subscribe(runId)
    expect(sse.handlers).toBeTruthy()

    // A stepProgress flush carrying BOTH an ordinary bar track and an
    // agent_activity track for the same step.
    sse.handlers.stepProgress({
      run_id: runId,
      step_id: 's1',
      tracks: [
        { id: 'dl', kind: { type: 'bar', fraction: 0.5 }, label: 'Downloading' },
        { id: 'a', kind: act(7, { title: 'Searching the web' }) },
      ],
    })

    const step = useWorkflowRunStore.getState().runs[runId].steps.s1
    // Agent activity landed in the dedicated timeline...
    expect(step.agentActivity?.map(e => e.seq)).toEqual([7])
    expect(step.agentActivity?.[0].title).toBe('Searching the web')
    // ...and did NOT pollute the generic track map, which keeps the bar track.
    expect(step.tracks?.dl?.kind).toEqual({ type: 'bar', fraction: 0.5 })
    expect(step.tracks?.a).toBeUndefined()
  })
})
