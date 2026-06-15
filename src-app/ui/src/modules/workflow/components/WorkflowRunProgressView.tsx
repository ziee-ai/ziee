import { StopOutlined } from '@ant-design/icons'
import {
  Alert,
  Button,
  Progress,
  Space,
  Spin,
  Steps,
  Tag,
  Typography,
} from 'antd'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'
import type { StepProgress } from '@/modules/workflow/stores/WorkflowRun.store'
import { StepArtifacts } from './StepArtifacts'
import { StepLogExpander } from './StepLogExpander'
import { StepOutputExpander } from './StepOutputExpander'
import { WorkflowElicitForm } from './WorkflowElicitForm'

const { Text } = Typography

interface WorkflowRunProgressViewProps {
  runId: string
}

function stepStatus(s: StepProgress): 'wait' | 'process' | 'finish' | 'error' {
  switch (s.status) {
    case 'running':
      return 'process'
    case 'completed':
      return 'finish'
    case 'failed':
      return 'error'
    default:
      return 'wait'
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

  useEffect(() => {
    Stores.WorkflowRun.subscribe(runId)
    return () => {
      Stores.WorkflowRun.unsubscribe(runId)
    }
  }, [runId])

  if (!run) {
    return <Spin />
  }

  const terminal = ['completed', 'failed', 'cancelled'].includes(run.status)
  const steps = run.stepOrder.map(id => run.steps[id])

  const statusColor =
    run.status === 'completed'
      ? 'green'
      : run.status === 'failed'
        ? 'red'
        : run.status === 'cancelled'
          ? 'default'
          : 'blue'

  return (
    <div className="flex flex-col gap-3">
      <Space align="center" wrap>
        <Tag color={statusColor}>{run.status}</Tag>
        <Text type="secondary" className="text-xs">
          {run.totalTokens.toLocaleString()} tokens
        </Text>
        {!run.connected && !terminal && (
          <Text type="warning" className="text-xs">
            reconnecting…
          </Text>
        )}
        {!terminal && (
          <Button
            danger
            size="small"
            icon={<StopOutlined />}
            loading={cancelling}
            onClick={() => void Stores.WorkflowRun.cancel(runId)}
          >
            Cancel
          </Button>
        )}
      </Space>

      {run.error && <Alert type="error" title={run.error} showIcon />}

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

      <Steps
        orientation="vertical"
        size="small"
        items={steps.map(s => ({
          status: stepStatus(s),
          title: (
            <Space size={8}>
              <Text>{s.message || s.stepId}</Text>
              {s.stepKind && <Tag className="text-xs !m-0">{s.stepKind}</Tag>}
            </Space>
          ),
          description: (
            <div className="flex flex-col gap-1">
              {s.itemProgress && s.itemProgress.total > 0 && (
                <Progress
                  size="small"
                  percent={Math.round(
                    ((s.itemProgress.completed + s.itemProgress.failed) /
                      s.itemProgress.total) *
                      100,
                  )}
                  status={s.itemProgress.failed > 0 ? 'exception' : undefined}
                  format={() =>
                    `${s.itemProgress!.completed}/${s.itemProgress!.total}` +
                    (s.itemProgress!.failed > 0
                      ? ` (${s.itemProgress!.failed} failed)`
                      : '')
                  }
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
                <Space size={4} wrap>
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
                </Space>
              )}
            </div>
          ),
        }))}
      />

      {steps.length === 0 && !terminal && (
        <Text type="secondary" className="text-xs">
          Waiting for steps to start…
        </Text>
      )}
    </div>
  )
}
