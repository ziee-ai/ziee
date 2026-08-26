/**
 * TEST-5 — component harness for the terminal-run detail region
 * (`BackgroundRunDetailTabs`, exported from `BackgroundRunCard.tsx`).
 *
 * Proves ITEM-3's discoverable transcript view in isolation, without the store:
 * the region is pure over its `run` + `detail` props (it reuses the shared
 * `AgentActivityTimeline` and `BackgroundRunResult`, neither of which reads the
 * BackgroundRuns store). Mounted with React's own `createRoot` + `act` (no
 * @testing-library), the same pattern as `JsToolApprovalContent.test.tsx`.
 *
 *   npx vitest run src/modules/background/components/BackgroundRunCard.test.tsx
 */
import { afterEach, beforeEach, describe, expect, test } from 'vitest'
import { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import type { BackgroundRunDetail, BackgroundRunSummary } from '@/api-client/types'
import { BackgroundRunDetailTabs } from './BackgroundRunCard'

;(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true

const RUN_ID = 'b0000000-0000-0000-0000-0000000000aa'

const run: BackgroundRunSummary = {
  id: RUN_ID,
  job_kind: 'subagent',
  label: 'Summarize the tickets',
  status: 'completed',
  has_result: true,
  total_tokens: 1234,
  created_at: '2026-01-03T09:10:00.000Z',
  updated_at: '2026-01-03T09:18:00.000Z',
}

const detailWithTranscript: BackgroundRunDetail = {
  ...run,
  final_output_json: { final_text: 'THE FINAL ANSWER', tokens_used: 1234 },
  total_tokens: 1234,
  activity: [
    { type: 'agent_activity', seq: 0, kind: 'thinking', title: 'Planning', status: 'ok' },
    {
      type: 'agent_activity',
      seq: 1,
      kind: 'tool_call',
      tool: 'search_knowledge',
      title: 'Searching',
      status: 'ok',
    },
    { type: 'agent_activity', seq: 2, kind: 'message', title: 'Wrote answer', status: 'ok' },
  ] as BackgroundRunDetail['activity'],
}

const detailNoTranscript: BackgroundRunDetail = {
  ...run,
  final_output_json: { final_text: 'THE FINAL ANSWER', tokens_used: 1234 },
  activity: [],
}

let container: HTMLDivElement
let root: Root

function mount(node: React.ReactElement) {
  act(() => {
    root.render(node)
  })
}

function click(el: Element | null) {
  if (!el) throw new Error('click target not found')
  act(() => {
    ;(el as HTMLElement).dispatchEvent(
      new MouseEvent('click', { bubbles: true, cancelable: true }),
    )
  })
}

const q = (testid: string) => container.querySelector(`[data-testid="${testid}"]`)
const qa = (selector: string) => container.querySelectorAll(selector)

beforeEach(() => {
  container = document.createElement('div')
  document.body.appendChild(container)
  root = createRoot(container)
})

afterEach(() => {
  act(() => root.unmount())
  container.remove()
})

describe('BackgroundRunDetailTabs (ITEM-3)', () => {
  test('defaults to the Transcript tab and renders one row per activity entry', () => {
    mount(<BackgroundRunDetailTabs run={run} detail={detailWithTranscript} />)

    // The named Transcript tab exists and is the discoverable affordance.
    const transcriptTab = q(`background-run-detail-tabs-${RUN_ID}-tab-transcript`)
    expect(transcriptTab).toBeTruthy()
    expect(transcriptTab?.textContent).toContain('Transcript')

    // Shared workflow timeline is what draws it (reuse, not a bespoke copy).
    expect(q('wf-activity-timeline-agent')).toBeTruthy()
    expect(qa('[data-testid^="wf-activity-row-agent-"]').length).toBe(3)

    // Result body is NOT mounted until its tab is selected.
    expect(q(`background-run-final-text-${RUN_ID}`)).toBeFalsy()
  })

  test('clicking the Result tab renders the final-text result body', () => {
    mount(<BackgroundRunDetailTabs run={run} detail={detailWithTranscript} />)
    click(q(`background-run-detail-tabs-${RUN_ID}-tab-result`))
    expect(q(`background-run-final-text-${RUN_ID}`)?.textContent).toContain(
      'THE FINAL ANSWER',
    )
  })

  test('a detail with no activity shows the empty note; Result tab still renders', () => {
    mount(<BackgroundRunDetailTabs run={run} detail={detailNoTranscript} />)

    // Transcript tab shows the friendly empty note, not a blank tab or a timeline.
    expect(q(`background-run-transcript-empty-${RUN_ID}`)).toBeTruthy()
    expect(q('wf-activity-timeline-agent')).toBeFalsy()

    // Positive control: the detail loaded, so the Result tab still renders.
    click(q(`background-run-detail-tabs-${RUN_ID}-tab-result`))
    expect(q(`background-run-final-text-${RUN_ID}`)?.textContent).toContain(
      'THE FINAL ANSWER',
    )
  })
})
