import { Button, Accordion, Spin, Paragraph, message } from '@/components/ui'
import { useState } from 'react'
import { ApiClient } from '@/api-client'

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
    <Accordion
      data-testid={`wf-step-log-accordion-${stepId}-${kind}`}
      ghost
      collapsible
      value={open ? kind : ''}
      onValueChange={(keys: string) => {
        const next = keys.length > 0
        setOpen(next)
        if (next && content === null && !loading) void fetchLog()
      }}
      items={[
        {
          key: kind,
          label: (
            <Button data-testid={`wf-step-log-btn-${stepId}-${kind}`} variant="link" size="sm" className="!px-0">
              {label}
            </Button>
          ),
          children: loading ? (
            <Spin size="sm" label="Loading log" />
          ) : error ? (
            <Paragraph data-testid="wf-step-log-empty" type="secondary" className="text-xs">
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
