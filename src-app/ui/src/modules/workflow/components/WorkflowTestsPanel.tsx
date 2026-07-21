import { CircleCheck, CircleMinus, CircleX } from 'lucide-react'
import { Alert, List, Space, Spin, Tag, Text } from '@ziee/kit'
import { Dialog } from '@ziee/kit'
import { useEffect, useState } from 'react'
import type { TestRunResponse, Workflow } from '@/api-client/types'
import { Workflow as WorkflowStore } from '@/modules/workflow/stores/workflow'

interface WorkflowTestsPanelProps {
  workflow: Workflow
  open: boolean
  onClose: () => void
}

/**
 * Runs `POST /test` (the bundle's `tests/` fixtures) and shows
 * per-fixture pass/fail. Available for dev-imported workflows where
 * `tests/` is present.
 */
export function WorkflowTestsPanel({
  workflow,
  open,
  onClose,
}: WorkflowTestsPanelProps) {
  const [loading, setLoading] = useState(false)
  const [result, setResult] = useState<TestRunResponse | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!open) return
    let cancelled = false
    setLoading(true)
    setError(null)
    setResult(null)
    WorkflowStore
      .test(workflow.id)
      .then(r => {
        if (!cancelled) setResult(r)
      })
      .catch(e => {
        if (!cancelled)
          setError(e instanceof Error ? e.message : 'Test run failed')
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
      data-testid="wf-tests-dialog"
      open={open}
      title="Workflow tests"
      onOpenChange={(v) => { if (!v) onClose() }}
      footer={null}
      className="!max-w-[640px]"
    >
      {loading && <Spin label="Loading" />}
      {error && <Alert data-testid="wf-tests-error-alert" tone="error" title={error} />}
      {result && (
        <div className="flex flex-col gap-3">
          <Space>
            <Tag variant="outline" data-testid="wf-tests-passed-tag" tone="success">{result.passed} passed</Tag>
            {result.failed > 0 && <Tag variant="outline" data-testid="wf-tests-failed-tag" tone="error">{result.failed} failed</Tag>}
            {result.skipped > 0 && (
              <Tag variant="outline" data-testid="wf-tests-skipped-tag">{result.skipped} skipped</Tag>
            )}
          </Space>
          <List
            data-testid="wf-tests-list"
            size="sm"
            rowKey="name"
            dataSource={result.results}
            renderItem={(r) => (
              <div className="list-item">
                <div className="flex items-start gap-3">
                  <div className="mt-1">
                    {r.skipped ? (
                      <CircleMinus className="text-muted-foreground text-lg" />
                    ) : r.passed ? (
                      <CircleCheck className="text-success text-lg" />
                    ) : (
                      <CircleX className="text-destructive text-lg" />
                    )}
                  </div>
                  <div className="flex-1">
                    <Text strong className="text-sm">{r.name}</Text>
                    {r.failure ? (
                      <div className="flex flex-col mt-1">
                        <Text type="danger" className="text-xs">
                          {r.failure.output_name}: {r.failure.assertion}
                        </Text>
                        <Text type="secondary" className="text-xs">
                          expected {r.failure.expected}; got{' '}
                          {r.failure.actual_preview}
                        </Text>
                      </div>
                    ) : (
                      <Text type="secondary" className="text-xs">
                        {r.duration_ms} ms
                      </Text>
                    )}
                  </div>
                </div>
              </div>
            )}
          />
        </div>
      )}
    </Dialog>
  )
}
