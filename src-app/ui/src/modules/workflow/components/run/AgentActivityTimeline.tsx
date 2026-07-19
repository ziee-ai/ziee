import { Accordion, Badge, type BadgeTone, Button, Card, Text } from '@ziee/kit'
import { useState } from 'react'
import type {
  AgentActivityStatus,
  SSEElicitationRequiredData,
} from '@/api-client/types'
import { ToolStatusIcon } from '@/modules/chat/core/ToolStatusIcon'
import type { ToolStatusKey } from '@/modules/chat/core/tool-status'
import { WorkflowElicitForm } from '../WorkflowElicitForm'
import { type AgentActivityEntry, describeActivity } from './activityDescriptors'

/** How many of the most-recent rows to render before collapsing the head of a
 *  long run behind a "Show all" affordance (keeps the DOM bounded). */
const MAX_VISIBLE = 40

/** Map the backend `AgentActivityStatus` onto a canonical tool-status glyph key
 *  + a soft badge tone + a human pill label. `ok`/`error` don't collide with the
 *  `running` in-flight spinner (the ToolStatusIcon vocabulary owns the color). */
function statusView(status: AgentActivityStatus): {
  toolKey: ToolStatusKey
  tone: BadgeTone
  label: string
} {
  switch (status) {
    case 'ok':
      return { toolKey: 'success', tone: 'success', label: 'Done' }
    case 'error':
      return { toolKey: 'failed', tone: 'error', label: 'Error' }
    case 'running':
    default:
      return { toolKey: 'running', tone: 'primary', label: 'Running' }
  }
}

/** One accreting editorial row: status glyph + domain-language line + a
 *  right-aligned status pill (wraps UNDER the line at narrow widths) + an
 *  optional "Show details" disclosure revealing the underlying tool + detail. */
function ActivityRow({
  stepId,
  entry,
}: {
  stepId: string
  entry: AgentActivityEntry
}) {
  const [open, setOpen] = useState(false)
  const view = statusView(entry.status)
  const line = describeActivity(entry)
  const detail = (entry.detail ?? '').trim()
  const tool = (entry.tool ?? '').trim()
  const hasDetails = detail.length > 0 || tool.length > 0
  const rowKey = `${entry.seq}`

  return (
    <div
      data-testid={`wf-activity-row-${stepId}-${entry.seq}`}
      className="flex items-start gap-2 py-1"
    >
      <span className="mt-0.5">
        <ToolStatusIcon status={view.toolKey} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        {/* Title + pill share one wrapping row: the pill is pushed to the end
            with `ms-auto` and drops UNDER the title (never off-screen) at 390px. */}
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
          {/* `min-w-0` lets the flex child shrink; `break-words` +
              overflow-wrap:anywhere break a long unbroken token (a fetched
              URL/DOI in a backend title) so it wraps inside the row instead of
              forcing horizontal page scroll at 390px. */}
          <Text className="min-w-0 break-words [overflow-wrap:anywhere] text-sm">
            {line}
          </Text>
          <Badge
            data-testid={`wf-activity-status-${stepId}-${entry.seq}`}
            tone={view.tone}
            className="ms-auto text-end"
          >
            {view.label}
          </Badge>
        </div>
        {hasDetails && (
          <Accordion
            data-testid={`wf-activity-details-${stepId}-${entry.seq}`}
            ghost
            collapsible
            value={open ? rowKey : ''}
            onValueChange={(v: string) => setOpen(v.length > 0)}
            items={[
              {
                key: rowKey,
                label: (
                  <span className="text-xs text-muted-foreground">
                    Show details
                  </span>
                ),
                children: (
                  <div className="flex flex-col gap-1">
                    {tool && (
                      <Text type="secondary" className="text-xs font-mono">
                        {tool}
                      </Text>
                    )}
                    {detail && (
                      <Text
                        type="secondary"
                        className="text-xs whitespace-pre-wrap"
                      >
                        {detail}
                      </Text>
                    )}
                  </div>
                ),
              },
            ]}
          />
        )}
      </div>
    </div>
  )
}

interface AgentActivityTimelineProps {
  stepId: string
  /** Ordered (by seq) activity rows for this step. */
  entries: AgentActivityEntry[]
  /** The run's pending human gate, when this step owns it — rendered inline so
   *  the scientist can answer without leaving the timeline. */
  elicitation?: SSEElicitationRequiredData
  submitting?: boolean
  onSubmitElicitation?: (response: Record<string, unknown>) => void
}

/**
 * The agent ACTIVITY TIMELINE: a scrolling, accreting list of domain-language
 * rows ("Searching the literature ✓ / Reading 3 papers ✓ / Drafting a summary …
 * running") replacing the single collapsing log line. A human gate renders
 * inline via {@link WorkflowElicitForm} so the run can be resumed in place.
 */
export function AgentActivityTimeline({
  stepId,
  entries,
  elicitation,
  submitting,
  onSubmitElicitation,
}: AgentActivityTimelineProps) {
  const [showAll, setShowAll] = useState(false)

  // The store (WorkflowRun.store `mergeAgentActivity`) already keeps this array
  // seq-ordered, so we trust that invariant and skip the per-render copy+sort
  // that otherwise ran on every SSE frame (FIX-3).
  const ordered = entries
  const overflow = ordered.length - MAX_VISIBLE
  const visible = showAll || overflow <= 0 ? ordered : ordered.slice(-MAX_VISIBLE)

  // Anchor the inline gate to the pending gate row when present, else fall back
  // to appending it after the list (the elicitation IS the gate either way).
  const gateSeq = ordered.find(
    e => e.kind === 'gate' && e.status === 'running',
  )?.seq
  const showGateForm = !!elicitation && !!onSubmitElicitation
  let gateRendered = false

  const gateForm = elicitation && onSubmitElicitation && (
    <WorkflowElicitForm
      elicitation={elicitation}
      submitting={submitting ?? false}
      onSubmit={onSubmitElicitation}
    />
  )

  return (
    <Card
      data-testid={`wf-activity-timeline-${stepId}`}
      size="sm"
      className="bg-card"
    >
      <div className="flex flex-col">
        {overflow > 0 && !showAll && (
          <div className="flex items-center gap-2 pb-1">
            <Text type="secondary" className="text-xs">
              Showing latest {MAX_VISIBLE} of {ordered.length}
            </Text>
            <Button
              data-testid={`wf-activity-show-all-${stepId}`}
              variant="link"
              size="default"
              onClick={() => setShowAll(true)}
            >
              Show all
            </Button>
          </div>
        )}
        {visible.map(entry => {
          const isGateAnchor =
            showGateForm && gateSeq != null && entry.seq === gateSeq
          if (isGateAnchor) gateRendered = true
          return (
            <div key={entry.seq}>
              <ActivityRow stepId={stepId} entry={entry} />
              {isGateAnchor && <div className="ps-6 py-1">{gateForm}</div>}
            </div>
          )
        })}
        {/* Gate arrived but no matching activity row (yet) → append it inline. */}
        {showGateForm && !gateRendered && (
          <div className="ps-6 py-1">{gateForm}</div>
        )}
      </div>
    </Card>
  )
}
