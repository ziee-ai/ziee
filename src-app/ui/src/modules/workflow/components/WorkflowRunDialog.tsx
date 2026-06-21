import { Alert, App, Form, Input, Modal, Select, Switch, Typography } from 'antd'
import { useEffect, useMemo, useState } from 'react'
import type { Workflow } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { parseWorkflowIr } from './workflowIr'

const { Text } = Typography

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
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [jsonInputs, setJsonInputs] = useState('{}')
  const [submitting, setSubmitting] = useState(false)
  const [jsonError, setJsonError] = useState<string | null>(null)
  const [modelId, setModelId] = useState<string | undefined>(undefined)
  const [captureLogs, setCaptureLogs] = useState(false)

  const { providers, selectedModelId } = Stores.ModelPicker

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
  // doesn't surface the prior run's values.
  useEffect(() => {
    if (!open) return
    form.resetFields()
    setJsonInputs('{}')
    setJsonError(null)
    setModelId(selectedModelId ?? undefined)
    setCaptureLogs(false)
  }, [open, workflow.id, form, selectedModelId])

  const handleRun = async () => {
    let inputValues: Record<string, unknown> = {}
    if (structured) {
      try {
        const values = await form.validateFields()
        inputValues = values
      } catch {
        return
      }
    } else {
      try {
        inputValues = JSON.parse(jsonInputs || '{}')
        setJsonError(null)
      } catch {
        setJsonError('Inputs must be valid JSON')
        return
      }
    }

    if (!conversationId && !modelId) {
      message.error('Select a model to run this workflow')
      return
    }

    setSubmitting(true)
    try {
      const res = await Stores.Workflow.run(
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

  return (
    <Modal
      open={open}
      title={`Run ${workflow.display_name || workflow.name}`}
      onCancel={onClose}
      onOk={handleRun}
      okText="Run"
      confirmLoading={submitting}
    >
      {structured ? (
        <Form form={form} layout="vertical">
          {inputs.map(input => (
            <Form.Item
              key={input.name}
              name={input.name}
              label={input.name}
              extra={input.description}
              rules={
                input.required
                  ? [{ required: true, message: `${input.name} is required` }]
                  : undefined
              }
              initialValue={input.default}
            >
              <Input placeholder={input.description} />
            </Form.Item>
          ))}
        </Form>
      ) : (
        <div className="flex flex-col gap-2">
          <Text type="secondary" className="text-xs">
            Provide inputs as a JSON object.
          </Text>
          <Input.TextArea
            rows={6}
            value={jsonInputs}
            onChange={e => setJsonInputs(e.target.value)}
            placeholder='{ "topic": "quantum entanglement" }'
          />
          {jsonError && <Alert type="error" title={jsonError} showIcon />}
        </div>
      )}
      {!conversationId && (
        <div className="mt-3 flex flex-col gap-1">
          <Text className="text-xs" id="workflow-run-model-label">
            Model
          </Text>
          <Select
            aria-label="Model"
            value={modelId}
            onChange={setModelId}
            options={modelOptions}
            placeholder="Select a model"
            showSearch
            optionFilterProp="label"
            popupMatchSelectWidth={false}
          />
        </div>
      )}
      <div className="mt-2 flex items-center gap-2">
        <Switch checked={captureLogs} onChange={setCaptureLogs} size="small" />
        <Text type="secondary" className="text-xs">
          Capture debug logs (prompts + raw output) for this run
        </Text>
      </div>
      {conversationId && (
        <div className="mt-2 flex items-center gap-2">
          <Switch checked disabled size="small" />
          <Text type="secondary" className="text-xs">
            Output posts back to the current conversation
          </Text>
        </div>
      )}
    </Modal>
  )
}
