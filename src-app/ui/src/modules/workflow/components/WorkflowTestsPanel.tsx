import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  MinusCircleOutlined,
} from '@ant-design/icons'
import { Alert, List, Modal, Space, Spin, Tag, Typography } from 'antd'
import { useEffect, useState } from 'react'
import type { TestRunResponse, Workflow } from '@/api-client/types'
import { Stores } from '@/core/stores'

const { Text } = Typography

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
    <Modal
      open={open}
      title="Workflow tests"
      onCancel={onClose}
      footer={null}
      width={640}
    >
      {loading && <Spin />}
      {error && <Alert type="error" title={error} showIcon />}
      {result && (
        <div className="flex flex-col gap-3">
          <Space>
            <Tag color="green">{result.passed} passed</Tag>
            {result.failed > 0 && <Tag color="red">{result.failed} failed</Tag>}
            {result.skipped > 0 && (
              <Tag color="default">{result.skipped} skipped</Tag>
            )}
          </Space>
          <List
            size="small"
            dataSource={result.results}
            renderItem={r => (
              <List.Item>
                <List.Item.Meta
                  avatar={
                    r.skipped ? (
                      <MinusCircleOutlined style={{ color: '#999' }} />
                    ) : r.passed ? (
                      <CheckCircleOutlined style={{ color: '#52c41a' }} />
                    ) : (
                      <CloseCircleOutlined style={{ color: '#ff4d4f' }} />
                    )
                  }
                  title={r.name}
                  description={
                    r.failure ? (
                      <div className="flex flex-col">
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
                    )
                  }
                />
              </List.Item>
            )}
          />
        </div>
      )}
    </Modal>
  )
}
