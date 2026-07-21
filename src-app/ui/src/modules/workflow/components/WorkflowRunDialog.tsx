import { CheckCircle } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import type { Workflow } from '@/api-client/types'
import { NEW_CHAT_MODEL_KEY } from '@/modules/user-llm-providers/modelPicker'
import {
  message,
  Dialog,
  Form,
  FormField,
  useForm,
  zodResolver,
  Button,
  Alert,
  Text,
  Select,
  Switch,
  Input,
  Textarea,
} from '@ziee/kit'
import { z } from 'zod'
import { parseWorkflowIr } from './workflowIr'
import { ModelPicker } from '@/modules/user-llm-providers/modelPicker'
import { Workflow as WorkflowStore } from '@/modules/workflow/stores/workflow'

interface WorkflowRunDialogProps {
  workflow: Workflow
  open: boolean
  onClose: () => void
  conversationId?: string
  /** Called with the new run id once the run is kicked off. */
  onStarted: (runId: string) => void
}

/**
 * Collects the workflow's inputs and kicks `POST /run`. When the
 * compiled IR exposes `inputs[]` we render a typed field per input;
 * otherwise we fall back to a free-form JSON editor.
 */
export function WorkflowRunDialog({
  workflow,
  open,
  onClose,
  conversationId,
  onStarted,
}: WorkflowRunDialogProps) {
  // Dynamic required-field validation for the workflow's typed inputs: the kit
  // FormField's `required` prop only decorates the label (aria-required + a
  // visual *), so the actual rule + "<name> is required" message must come from
  // the form resolver.
  const inputsForSchema = useMemo(() => parseWorkflowIr(workflow).inputs, [workflow])
  const inputSchema = useMemo(
    () =>
      z.object(
        Object.fromEntries(
          inputsForSchema.map(i => [
            i.name,
            i.required
              ? z.string().min(1, `${i.name} is required`)
              : z.string().optional(),
          ]),
        ),
      ),
    [inputsForSchema],
  )
  const form = useForm<Record<string, unknown>>({
    // The schema is built from the workflow's dynamic inputs (all string fields),
    // so its inferred output is Record<string, string | undefined>; cast to the
    // form's Record<string, unknown> value type (validation is runtime).
    resolver: zodResolver(inputSchema) as unknown as import('react-hook-form').Resolver<
      Record<string, unknown>
    >,
    defaultValues: {},
  })
  const [jsonInputs, setJsonInputs] = useState('{}')
  const [submitting, setSubmitting] = useState(false)
  const [jsonError, setJsonError] = useState<string | null>(null)
  const [modelId, setModelId] = useState<string | undefined>(undefined)
  const [captureLogs, setCaptureLogs] = useState(false)

  const { providers, selectedByConversation, loading: modelsLoading } =
    ModelPicker
  // The general "current" default model (the new-chat selection) — the workflow
  // run dialog isn't pane-scoped, so it just needs a sensible default (ITEM-5).
  const selectedModelId = selectedByConversation[NEW_CHAT_MODEL_KEY]

  // Grouped model options from the user's accessible providers (used for a
  // standalone run, where the model isn't snapshotted from a conversation).
  const modelOptions = useMemo(
    () =>
      (providers || [])
        .map(p => ({
          label: p.name,
          options: (p.llm_models || [])
            .filter(m => m.enabled)
            .map(m => ({ label: m.display_name || m.name, value: m.id })),
        }))
        .filter(g => g.options.length > 0),
    [providers],
  )

  const { inputs } = useMemo(() => parseWorkflowIr(workflow), [workflow])
  const structured = inputs.length > 0

  // Reset the form + JSON editor each time the dialog opens (or the
  // target workflow changes) so reopening for a different workflow
  // doesn't surface the prior run's values. Keyed on open/workflow ONLY:
  // `selectedModelId` resolves asynchronously (models load after the dialog
  // opens), and including it here re-ran this reset mid-session, wiping the
  // user's in-progress JSON/form inputs back to defaults.
  useEffect(() => {
    if (!open) return
    form.reset(Object.fromEntries(inputs.map(i => [i.name, i.default ?? ''])))
    setJsonInputs('{}')
    setJsonError(null)
    setCaptureLogs(false)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, workflow.id])

  // Apply the incoming default model separately — models can resolve after the
  // dialog is already open, and this must NOT reset the user's typed inputs.
  useEffect(() => {
    if (!open) return
    setModelId(selectedModelId ?? undefined)
  }, [open, selectedModelId])

  const runWith = async (inputValues: Record<string, unknown>) => {
    if (!conversationId && !modelId) {
      message.error('Select a model to run this workflow')
      return
    }

    setSubmitting(true)
    try {
      const res = await WorkflowStore.run(
        workflow.id,
        inputValues,
        conversationId,
        undefined,
        modelId,
        captureLogs,
      )
      message.success('Workflow run started')
      onStarted(res.run_id)
      onClose()
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to start run')
    } finally {
      setSubmitting(false)
    }
  }

  const handleRun = () => {
    if (structured) {
      // handleSubmit runs the resolver AND marks the form submitted, so the
      // FormField surfaces "<name> is required" for empty required inputs
      // (its error is gated on isTouched || isSubmitted).
      void form.handleSubmit(values => runWith(values))()
    } else {
      let inputValues: Record<string, unknown>
      try {
        inputValues = JSON.parse(jsonInputs || '{}')
        setJsonError(null)
      } catch {
        setJsonError('Inputs must be valid JSON')
        return
      }
      void runWith(inputValues)
    }
  }

  return (
    <Dialog
      data-testid="wf-run-dialog"
      open={open}
      onOpenChange={v => { if (!v) onClose() }}
      title={`Run ${workflow.display_name || workflow.name}`}
      footer={
        <>
          <Button data-testid="wf-run-cancel-btn" variant="outline" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button data-testid="wf-run-submit-btn" onClick={handleRun} loading={submitting}>
            Run
          </Button>
        </>
      }
    >
      {structured ? (
        <Form data-testid="wf-run-form" form={form} onSubmit={() => { void handleRun() }}>
          {inputs.map(input => (
            <FormField
              key={input.name}
              name={input.name}
              label={input.name}
              description={input.description}
              required={input.required}
            >
              <Input data-testid={`wf-run-input-${input.name}`} placeholder={input.description} />
            </FormField>
          ))}
        </Form>
      ) : (
        <div className="flex flex-col gap-2">
          <Text type="secondary" className="text-xs">
            Provide inputs as a JSON object.
          </Text>
          <Textarea
            data-testid="wf-run-json-textarea"
            rows={6}
            value={jsonInputs}
            onChange={e => setJsonInputs(e.target.value)}
            placeholder='{ "topic": "quantum entanglement" }'
          />
          {jsonError && <Alert data-testid="wf-run-json-error-alert" tone="error" title={jsonError} />}
        </div>
      )}
      {!conversationId && (
        <div className="mt-3 flex flex-col gap-1">
          <Text className="text-xs" id="workflow-run-model-label">
            Model
          </Text>
          <Select
            data-testid="wf-run-model-select"
            aria-label="Model"
            value={modelId}
            onChange={setModelId}
            options={modelOptions}
            // Spinner + aria-busy while the accessible providers/models resolve
            // (they load after the dialog opens); once loaded, an empty list
            // surfaces the placeholder rather than a stuck empty control.
            loading={modelsLoading && modelOptions.length === 0}
            placeholder={modelsLoading && modelOptions.length === 0 ? 'Loading models…' : 'Select a model'}
            popupMatchSelectWidth={false}
          />
        </div>
      )}
      <div className="mt-2 flex items-center gap-2">
        <Switch tooltip="Capture debug logs for this run" data-testid="wf-run-capture-logs-switch" checked={captureLogs} onChange={setCaptureLogs} size="sm" />
        <Text type="secondary" className="text-xs">
          Capture debug logs (prompts + raw output) for this run
        </Text>
      </div>
      {conversationId && (
        <div className="mt-2 flex items-center gap-2">
          <CheckCircle className="size-4 text-success" aria-hidden />
          <Text type="secondary" className="text-xs">
            Output posts back to the current conversation
          </Text>
        </div>
      )}
    </Dialog>
  )
}
