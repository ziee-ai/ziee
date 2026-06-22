import { App, Button, Collapse, Spin, Typography } from 'antd'
import { useState } from 'react'
import { ApiClient } from '@/api-client'

const { Paragraph } = Typography

interface StepLogExpanderProps {
  runId: string
  stepId: string
  /** Log kind: 'prompt' | 'raw_output' | 'stderr' | 'trace' (per the log endpoint). */
  kind: string
  label: string
}

/**
 * Lazily fetches a step's log resource (`GET /workflow-runs/{run}/logs/
 * {step}/{kind}`) and shows it inline. Only mount this when the log is
 * known to be exposed (dev / expose_logs allows) — a 404 just shows an
 * "unavailable" note.
 */
export function StepLogExpander({
  runId,
  stepId,
  kind,
  label,
}: StepLogExpanderProps) {
  const { message } = App.useApp()
  const [open, setOpen] = useState(false)
  const [loading, setLoading] = useState(false)
  const [content, setContent] = useState<string | null>(null)
  const [error, setError] = useState(false)

  const fetchLog = async () => {
    setLoading(true)
    setError(false)
    try {
      const res = await ApiClient.Workflow.readLog({
        run_id: runId,
        step_id: stepId,
        kind,
      })
      setContent(typeof res === 'string' ? res : JSON.stringify(res, null, 2))
    } catch (e) {
      // 404 is the EXPECTED "log not exposed / not produced" case — show
      // the inline "not available" note. Any other failure (network,
      // 403, 5xx) is a real error → surface it so it isn't masked by the
      // expected-404 message.
      setError(true)
      const status =
        typeof e === 'object' && e !== null
          ? (e as { status?: number }).status
          : undefined
      if (status !== 404) {
        message.error(
          e instanceof Error ? e.message : `Failed to load ${label}`,
        )
      }
    } finally {
      setLoading(false)
    }
  }

  return (
    <Collapse
      ghost
      size="small"
      activeKey={open ? [kind] : []}
      onChange={keys => {
        const next = keys.length > 0
        setOpen(next)
        if (next && content === null && !loading) void fetchLog()
      }}
      items={[
        {
          key: kind,
          label: (
            <Button type="link" size="small" className="!px-0">
              {label}
            </Button>
          ),
          children: loading ? (
            <Spin size="small" />
          ) : error ? (
            <Paragraph type="secondary" className="text-xs">
              Log not available
            </Paragraph>
          ) : (
            <Paragraph className="text-xs whitespace-pre-wrap !mb-0">
              {content}
            </Paragraph>
          ),
        },
      ]}
    />
  )
}
