import { Collapse, Spin, Typography } from 'antd'
import { useState } from 'react'
import { Streamdown } from 'streamdown'
import { ApiClient } from '@/api-client'
import { StreamdownErrorBoundary } from '@/modules/chat/core/utils/StreamdownErrorBoundary'

const { Paragraph } = Typography

interface StepOutputExpanderProps {
  runId: string
  stepId: string
  /** From the step's output metadata; selects how the body renders. */
  parsedAs?: 'json' | 'text'
}

/**
 * "Show full output" expander for a completed step. The SSE stream only
 * carries a 500-char preview; this lazily fetches the full output bytes
 * via `GET /api/workflow-runs/{run}/output/{step}` (§4.4 "Show full
 * output") and renders them — markdown via Streamdown for text outputs,
 * a fenced code block for JSON. A 404 (step not yet completed / file
 * cleaned up) shows an "unavailable" note.
 */
export function StepOutputExpander({
  runId,
  stepId,
  parsedAs,
}: StepOutputExpanderProps) {
  const [open, setOpen] = useState(false)
  const [loading, setLoading] = useState(false)
  const [content, setContent] = useState<string | null>(null)
  const [isJson, setIsJson] = useState(parsedAs === 'json')
  const [error, setError] = useState(false)

  const fetchOutput = async () => {
    setLoading(true)
    setError(false)
    try {
      // readOutput returns text (text/plain) or a parsed object
      // (application/json) depending on the response Content-Type, which
      // the server derives from the step's `parsed_as`.
      const res = await ApiClient.Workflow.readOutput({
        run_id: runId,
        step_id: stepId,
      })
      if (typeof res === 'string') {
        setContent(res)
        setIsJson(parsedAs === 'json')
      } else {
        setContent(JSON.stringify(res, null, 2))
        setIsJson(true)
      }
    } catch {
      setError(true)
    } finally {
      setLoading(false)
    }
  }

  const body = () => {
    if (loading) return <Spin size="small" />
    if (error) {
      return (
        <Paragraph type="secondary" className="text-xs !mb-0">
          Output not available
        </Paragraph>
      )
    }
    if (content === null) return null
    if (isJson) {
      return (
        <pre className="p-2 rounded overflow-auto max-h-80 text-xs !mb-0">
          {content}
        </pre>
      )
    }
    return (
      <div className="overflow-auto max-h-80 text-sm">
        <StreamdownErrorBoundary fallbackText={content}>
          <Streamdown shikiTheme={['github-light', 'github-dark']}>
            {content}
          </Streamdown>
        </StreamdownErrorBoundary>
      </div>
    )
  }

  return (
    <Collapse
      ghost
      size="small"
      activeKey={open ? ['output'] : []}
      onChange={keys => {
        const next = keys.length > 0
        setOpen(next)
        if (next && content === null && !loading) void fetchOutput()
      }}
      items={[
        {
          key: 'output',
          label: (
            <span className="text-xs text-[var(--ant-color-link)]">
              Show full output
            </span>
          ),
          children: body(),
        },
      ]}
    />
  )
}
