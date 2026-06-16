import { Alert, Modal, Statistic, Table, Typography } from 'antd'
import { useEffect, useState } from 'react'
import type { DryRunResult, DryRunStep, Workflow } from '@/api-client/types'
import { Stores } from '@/core/stores'

const { Text } = Typography

interface DryRunPreviewDialogProps {
  workflow: Workflow
  open: boolean
  onClose: () => void
}

/**
 * Calls `POST /dry-run` and shows per-step estimated call/token counts
 * (and total est cost) before the user commits to a real run. Spends
 * zero tokens.
 */
export function DryRunPreviewDialog({
  workflow,
  open,
  onClose,
}: DryRunPreviewDialogProps) {
  const [loading, setLoading] = useState(false)
  const [result, setResult] = useState<DryRunResult | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!open) return
    let cancelled = false
    setLoading(true)
    setError(null)
    setResult(null)
    Stores.Workflow.__state
      .dryRun(workflow.id, {})
      .then(r => {
        if (!cancelled) setResult(r)
      })
      .catch(e => {
        if (!cancelled)
          setError(e instanceof Error ? e.message : 'Dry-run failed')
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [open, workflow.id])

  return (
    <Modal
      open={open}
      title="Dry-run preview"
      onCancel={onClose}
      footer={null}
      width={640}
    >
      {error && <Alert type="error" title={error} showIcon />}
      {result && (
        <div className="flex flex-col gap-3">
          <div className="flex gap-6">
            <Statistic title="Est. calls" value={result.total_est_calls} />
            <Statistic title="Est. tokens" value={result.total_est_tokens} />
            {result.est_cost_usd != null && (
              <Statistic
                title="Est. cost"
                value={result.est_cost_usd}
                precision={4}
                prefix="$"
              />
            )}
          </div>
          <Table<DryRunStep>
            size="small"
            rowKey="step_id"
            loading={loading}
            pagination={false}
            dataSource={result.steps}
            columns={[
              { title: 'Step', dataIndex: 'step_id' },
              { title: 'Kind', dataIndex: 'kind' },
              { title: 'Calls', dataIndex: 'est_calls' },
              {
                title: 'Tokens (in/out)',
                render: (_: unknown, s: DryRunStep) =>
                  `${s.est_tokens_in} / ${s.est_tokens_out}`,
              },
              {
                title: '',
                render: (_: unknown, s: DryRunStep) =>
                  s.runtime_dependent ? (
                    <Text type="secondary" className="text-xs">
                      runtime-dependent
                    </Text>
                  ) : null,
              },
            ]}
          />
          <Text type="secondary" className="text-xs">
            Estimates only — fan-out (llm_map) counts depend on runtime data.
          </Text>
        </div>
      )}
    </Modal>
  )
}
