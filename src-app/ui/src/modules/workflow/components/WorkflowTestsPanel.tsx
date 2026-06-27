import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  MinusCircleOutlined,
} from '@ant-design/icons'
import { Alert, List, Space, Spin, Tag, Text } from '@/components/ui'
import { Dialog } from '@/components/ui'
import { useEffect, useState } from 'react'
import type { TestRunResponse, Workflow } from '@/api-client/types'
import { Stores } from '@/core/stores'

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
    Stores.Workflow.__state
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
      open={open}
      title="Workflow tests"
      onOpenChange={(v) => { if (!v) onClose() }}
      footer={null}
      className="!max-w-[640px]"
    >
      {loading && <Spin label="Loading" />}
      {error && <Alert tone="error" title={error} />}
      {result && (
        <div className="flex flex-col gap-3">
          <Space>
            <Tag tone="success">{result.passed} passed</Tag>
            {result.failed > 0 && <Tag tone="error">{result.failed} failed</Tag>}
            {result.skipped > 0 && (
              <Tag>{result.skipped} skipped</Tag>
            )}
          </Space>
          <List
            size="sm"
            rowKey="name"
            dataSource={result.results}
            renderItem={(r) => (
              <div className="list-item">
                <div className="flex items-start gap-3">
                  <div className="mt-1">
                    {r.skipped ? (
                      <MinusCircleOutlined className="text-[#999] text-lg" />
                    ) : r.passed ? (
                      <CheckCircleOutlined className="text-[#52c41a] text-lg" />
                    ) : (
                      <CloseCircleOutlined className="text-[#ff4d4f] text-lg" />
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
