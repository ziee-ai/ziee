import { AlertTriangle, CheckCircle2, XCircle } from 'lucide-react'
import { Spinner, Text } from '@ziee/kit'
import type { ValidationError } from '@/api-client/types'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'

interface BuilderValidationPanelProps {
  store: WorkflowBuilderStore
}

function Finding({
  finding,
  tone,
}: {
  finding: ValidationError
  tone: 'error' | 'warning'
}) {
  const Icon = tone === 'error' ? XCircle : AlertTriangle
  const color = tone === 'error' ? 'text-destructive' : 'text-warning'
  return (
    <li className="flex items-start gap-2">
      <Icon className={`size-4 mt-0.5 shrink-0 ${color}`} aria-hidden />
      <div className="flex flex-col">
        <Text className="text-sm">{finding.message}</Text>
        {finding.location && (
          <Text type="secondary" className="text-xs">
            at {finding.location}
          </Text>
        )}
      </div>
    </li>
  )
}

/** ITEM-7 — inline validation + cost estimate from `POST /validate-def`. Errors
 *  block Save (the page disables the button); warnings are surfaced but allowed. */
export function BuilderValidationPanel({ store }: BuilderValidationPanelProps) {
  const validation = store.validation
  const validating = store.validating
  const errors = validation?.errors ?? []
  const warnings = validation?.warnings ?? []
  const cost = validation?.cost_estimate

  return (
    <div className="flex flex-col gap-3" data-testid="wf-builder-validation">
      <div className="flex items-center gap-2">
        <Text strong>Validation</Text>
        {validating && <Spinner size="sm" label="Validating workflow" />}
      </div>

      {!validation && !validating && (
        <Text type="secondary" className="text-xs">
          Add steps to validate the workflow.
        </Text>
      )}

      {validation && errors.length === 0 && (
        <div className="flex items-center gap-2" data-testid="wf-builder-valid">
          <CheckCircle2 className="size-4 text-success" aria-hidden />
          <Text className="text-sm">No blocking errors.</Text>
        </div>
      )}

      {errors.length > 0 && (
        <ul className="flex flex-col gap-2" data-testid="wf-builder-errors">
          {errors.map((f, i) => (
            <Finding key={`e-${i}`} finding={f} tone="error" />
          ))}
        </ul>
      )}

      {warnings.length > 0 && (
        <ul className="flex flex-col gap-2" data-testid="wf-builder-warnings">
          {warnings.map((f, i) => (
            <Finding key={`w-${i}`} finding={f} tone="warning" />
          ))}
        </ul>
      )}

      {cost && (
        <div
          className="rounded-md bg-muted p-3 flex flex-col gap-1"
          data-testid="wf-builder-cost"
        >
          <Text className="text-xs font-medium text-muted-foreground">
            Estimated cost
          </Text>
          <Text className="text-sm">
            {cost.total_est_calls} model call
            {cost.total_est_calls === 1 ? '' : 's'} ·{' '}
            {cost.total_est_tokens.toLocaleString()} tokens
            {cost.est_cost_usd != null
              ? ` · ~$${cost.est_cost_usd.toFixed(2)}`
              : ''}
          </Text>
        </div>
      )}
    </div>
  )
}
