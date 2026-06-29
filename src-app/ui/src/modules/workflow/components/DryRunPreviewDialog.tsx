import { Alert, Dialog, Statistic, Table, Text } from '@/components/ui'
import { useEffect, useState } from 'react'
import type { DryRunResult, DryRunStep, Workflow } from '@/api-client/types'
import { Stores } from '@/core/stores'

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
    <Dialog
      data-testid="wf-dry-run-dialog"
      open={open}
      title="Dry-run preview"
      onOpenChange={(v) => { if (!v) onClose() }}
      footer={null}
      className="!max-w-[640px]"
    >
      {error && <Alert data-testid="wf-dry-run-error-alert" tone="error" title={error} />}
      {result && (
        <div className="flex flex-col gap-3">
          <div className="flex gap-6">
            <Statistic data-testid="wf-dry-run-stat-calls" title="Est. calls" value={result.total_est_calls} />
            <Statistic data-testid="wf-dry-run-stat-tokens" title="Est. tokens" value={result.total_est_tokens} />
            {result.est_cost_usd != null && (
              <Statistic
                data-testid="wf-dry-run-stat-cost"
                title="Est. cost"
                value={result.est_cost_usd}
                precision={4}
                prefix="$"
              />
            )}
          </div>
          <Table<DryRunStep>
            data-testid="wf-dry-run-steps-table"
            rowKey="step_id"
            loading={loading}
            dataSource={result.steps}
            columns={[
              { key: 'step', title: 'Step', dataIndex: 'step_id' },
              { key: 'kind', title: 'Kind', dataIndex: 'kind' },
              { key: 'calls', title: 'Calls', dataIndex: 'est_calls' },
              {
                key: 'tokens',
                title: 'Tokens (in/out)',
                render: (record: DryRunStep) =>
                  `${record.est_tokens_in} / ${record.est_tokens_out}`,
              },
              {
                key: 'runtime',
                title: '',
                render: (record: DryRunStep) =>
                  record.runtime_dependent ? (
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
    </Dialog>
  )
}
