import { Ban } from 'lucide-react'
import {
  message,
  Alert,
  Button,
  Progress,
  Space,
  Spin,
  Tag,
  Text,
} from '@/components/ui'
import { useEffect, useState } from 'react'
import { ApiClient } from '@/api-client'
import type { ProgressTrack } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { StepArtifacts } from './StepArtifacts'
import { StepLogExpander } from './StepLogExpander'
import { StepOutputExpander } from './StepOutputExpander'
import { WorkflowElicitForm } from './WorkflowElicitForm'

interface WorkflowRunProgressViewProps {
  runId: string
}

/** How many parallel tracks to render before collapsing to "+N more". */
const TRACK_DISPLAY_CAP = 12

/** Render one live sandbox-progress track (P2) by its kind. All strings are
 *  plaintext — React escapes them (the UI owns rendering, never the author). */
function TrackWidget({ track }: { track: ProgressTrack }) {
  const k = track.kind
  const label = track.label
  switch (k.type) {
    case 'bar':
      return (
        <Progress
          data-testid={`wf-track-progress-${track.id}`}
          size="sm"
          value={Math.round(k.fraction * 100)}
          format={label ? () => label : undefined}
          aria-label={label || 'Progress bar'}
        />
      )
    case 'counter': {
      const pct = k.total > 0 ? Math.round((k.current / k.total) * 100) : 0
      return (
        <Progress
          data-testid={`wf-track-progress-${track.id}`}
          size="sm"
          value={pct}
          format={() =>
            `${k.current}/${k.total}${k.unit ? ` ${k.unit}` : ''}` +
            (label ? ` · ${label}` : '')
          }
          aria-label={label || 'Counter progress'}
        />
      )
    }
    case 'status':
      return (
        <Text type="secondary" className="text-xs">
          {label ? `${label}: ` : ''}
          {k.message}
        </Text>
      )
    case 'log':
      return (
        <Text type="secondary" className="text-xs font-mono" ellipsis>
          {k.line}
        </Text>
      )
    case 'phase':
      return (
        <Text type="secondary" className="text-xs">
          {k.name}
          {k.index != null && k.total != null ? ` (${k.index}/${k.total})` : ''}
        </Text>
      )
    default:
      return null
  }
}

/**
 * Live run timeline: subscribes to the per-run SSE stream, renders the
 * step list (with per-item progress bars for llm_map fan-out), a
 * running token total, a Cancel button, an inline elicitation form when
 * a `kind: elicit` step is waiting, and per-step log expanders.
 */
export function WorkflowRunProgressView({
  runId,
}: WorkflowRunProgressViewProps) {
  const run = Stores.WorkflowRun.runs[runId]
  const cancelling = Stores.WorkflowRun.cancelling[runId] ?? false
  const submittingElicit = Stores.WorkflowRun.submittingElicit[runId] ?? false
  const [removingTimeout, setRemovingTimeout] = useState(false)

  useEffect(() => {
    Stores.WorkflowRun.subscribe(runId)
    return () => {
      Stores.WorkflowRun.unsubscribe(runId)
    }
  }, [runId])

  if (!run) {
    return <Spin label="Loading" />
  }

  const terminal = ['completed', 'failed', 'cancelled'].includes(run.status)
  const steps = run.stepOrder.map(id => run.steps[id])

  const tone =
    run.status === 'completed'
      ? 'success'
      : run.status === 'failed'
        ? 'error'
        : run.status === 'cancelled'
          ? undefined
          : // `waiting` = durably paused on a human gate (non-terminal); flag it
            // distinctly from the active `running`/`pending` blue.
            run.status === 'waiting'
            ? 'warning'
            : 'info'

  return (
    <div className="flex flex-col gap-3">
      <Space direction="horizontal" align="center" wrap>
        <Tag data-testid="wf-progress-status-tag" tone={tone}>{run.status}</Tag>
        <Text type="secondary" className="text-xs">
          {run.totalTokens.toLocaleString()} tokens
        </Text>
        {!run.connected && !terminal && (
          <Text data-testid="wf-progress-reconnecting" type="warning" className="text-xs">
            reconnecting…
          </Text>
        )}
        {!terminal && (
          <Button
            data-testid="wf-progress-cancel-btn"
            variant="destructive"
            size="sm"
            icon={<Ban />}
            loading={cancelling}
            onClick={() => void Stores.WorkflowRun.cancel(runId)}
          >
            Cancel
          </Button>
        )}
        {/* Lift the wall-clock cap mid-run (e.g. a long literature pass on your
            own machine). The per-run token/byte caps still apply. */}
        {!terminal && (
          <Button
            data-testid="wf-progress-remove-timeout-btn"
            size="sm"
            loading={removingTimeout}
            disabled={removingTimeout}
            onClick={async () => {
              setRemovingTimeout(true)
              try {
                const r = await ApiClient.Workflow.setRunTimeout({ run_id: runId, timeout_secs: 0 })
                if (r?.status === 'updated') {
                  message.success('Timeout removed — this run is no longer wall-clock limited')
                } else {
                  message.info('Run already finished — nothing to update')
                }
              } catch (e) {
                message.error(e instanceof Error ? e.message : 'Failed to update timeout')
              } finally {
                setRemovingTimeout(false)
              }
            }}
          >
            Remove timeout
          </Button>
        )}
      </Space>

      {run.error && <Alert data-testid="wf-progress-error-alert" tone="error" title={run.error} />}

      {run.pendingElicitation && (
        <WorkflowElicitForm
          elicitation={run.pendingElicitation}
          submitting={submittingElicit}
          onSubmit={response =>
            void Stores.WorkflowRun.submitElicitation(
              runId,
              run.pendingElicitation!.elicitation_id,
              response,
            )
          }
        />
      )}

      <div>
        {steps.map(s => (
          <div key={s.stepId} className="flex flex-col gap-2 py-2">
            <Space direction="horizontal" size={8}>
              <Text>{s.description || s.message || s.stepId}</Text>
              {s.stepKind && <Tag data-testid={`wf-progress-step-kind-tag-${s.stepId}`} className="text-xs !m-0">{s.stepKind}</Tag>}
            </Space>
            <div className="flex flex-col gap-1 ml-4">
              {s.tracks && Object.keys(s.tracks).length > 0 && (
                <div className="flex flex-col gap-0.5">
                  {Object.values(s.tracks)
                    .slice(0, TRACK_DISPLAY_CAP)
                    .map((t, i) => (
                      <TrackWidget key={t.id || `_${i}`} track={t} />
                    ))}
                  {Object.keys(s.tracks).length > TRACK_DISPLAY_CAP && (
                    <Text type="secondary" className="text-xs">
                      +{Object.keys(s.tracks).length - TRACK_DISPLAY_CAP} more
                    </Text>
                  )}
                </div>
              )}
              {s.itemProgress && s.itemProgress.total > 0 && (
                <Progress
                  data-testid={`wf-progress-item-${s.stepId}`}
                  size="sm"
                  value={Math.round(
                    ((s.itemProgress.completed + s.itemProgress.failed) /
                      s.itemProgress.total) *
                      100,
                  )}
                  tone={s.itemProgress.failed > 0 ? 'error' : 'primary'}
                  format={() =>
                    `${s.itemProgress!.completed}/${s.itemProgress!.total}` +
                    (s.itemProgress!.failed > 0
                      ? ` (${s.itemProgress!.failed} failed)`
                      : '')
                  }
                  aria-label="Item progress"
                />
              )}
              {s.outputPreview && (
                <Text type="secondary" className="text-xs" ellipsis>
                  {s.outputPreview}
                </Text>
              )}
              {s.error && (
                <Text type="danger" className="text-xs">
                  {s.error}
                </Text>
              )}
              {(s.tokensUsed != null || s.msElapsed != null) && (
                <Text type="secondary" className="text-xs">
                  {s.tokensUsed != null ? `${s.tokensUsed} tokens` : ''}
                  {s.tokensUsed != null && s.msElapsed != null ? ' · ' : ''}
                  {s.msElapsed != null
                    ? `${(s.msElapsed / 1000).toFixed(1)}s`
                    : ''}
                </Text>
              )}
              {s.status === 'completed' && s.hasOutput && (
                <StepOutputExpander
                  runId={runId}
                  stepId={s.stepId}
                  parsedAs={s.outputMeta?.parsed_as}
                />
              )}
              {s.artifacts && s.artifacts.length > 0 && (
                <StepArtifacts
                  runId={runId}
                  stepId={s.stepId}
                  artifacts={s.artifacts}
                />
              )}
              {(s.status === 'completed' || s.status === 'failed') && (
                <Space direction="horizontal" size={4} wrap>
                  <StepLogExpander
                    runId={runId}
                    stepId={s.stepId}
                    kind="prompt"
                    label="Show prompt"
                  />
                  <StepLogExpander
                    runId={runId}
                    stepId={s.stepId}
                    kind="raw_output"
                    label="Show raw output"
                  />
                  {/* stderr is only produced by sandbox steps. */}
                  {s.stepKind === 'sandbox' && (
                    <StepLogExpander
                      runId={runId}
                      stepId={s.stepId}
                      kind="stderr"
                      label="Show stderr"
                    />
                  )}
                  {/* trace.json is written only on completion, never on failure. */}
                  {s.status === 'completed' && (
                    <StepLogExpander
                      runId={runId}
                      stepId={s.stepId}
                      kind="trace"
                      label="Show trace"
                    />
                  )}
                </Space>
              )}
            </div>
          </div>
        ))}
      </div>

      {steps.length === 0 && !terminal && (
        <Text type="secondary" className="text-xs">
          Waiting for steps to start…
        </Text>
      )}
    </div>
  )
}
