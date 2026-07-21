import { Inbox } from 'lucide-react'
import { Alert, Button, Dialog, Space, Text, Upload, message } from '@ziee/kit'
import { useState } from 'react'
import type { ValidateWorkflowResponse } from '@/api-client/types'
import { Workflow as WorkflowStore } from '@/modules/workflow/stores/workflow'
import { SystemWorkflow } from '@/modules/workflow/stores/systemWorkflow'

interface ImportWorkflowDialogProps {
  open: boolean
  onClose: () => void
  /** When true, import as a system-scope workflow (admin). */
  system?: boolean
}

/**
 * Import a workflow from a local bundle (tar.gz of the source dir).
 * The bundle's workflow.yaml is validated server-side; an inline
 * /validate result is shown when a workflow.yaml is dropped directly.
 */
export function ImportWorkflowDialog({
  open,
  onClose,
  system,
}: ImportWorkflowDialogProps) {
  const [files, setFiles] = useState<File[]>([])
  const [validation, setValidation] = useState<ValidateWorkflowResponse | null>(
    null,
  )
  const [submitting, setSubmitting] = useState(false)
  const [validating, setValidating] = useState(false)

  const reset = () => {
    setFiles([])
    setValidation(null)
    setSubmitting(false)
    setValidating(false)
  }

  const handleValidate = async () => {
    const file = files[0]
    if (!file) {
      message.warning('Select a workflow.yaml or bundle to validate')
      return
    }
    if (!file.name.endsWith('.yaml') && !file.name.endsWith('.yml')) {
      message.info(
        'Validation reads workflow.yaml; drop a workflow.yaml to preview, or import the bundle directly.',
      )
      return
    }
    setValidating(true)
    try {
      const text = await file.text()
      setValidation(await WorkflowStore.validateWorkflow(text))
    } catch {
      message.error('Validation request failed')
    } finally {
      setValidating(false)
    }
  }

  const handleImport = async () => {
    const file = files[0]
    if (!file) {
      message.warning('Select a bundle to import')
      return
    }
    setSubmitting(true)
    try {
      const form = new FormData()
      form.append('bundle', file)
      if (system) form.append('scope', 'system')
      if (system) {
        await SystemWorkflow.importSystemWorkflow(form)
      } else {
        await WorkflowStore.importWorkflow(form)
      }
      message.success('Workflow imported')
      reset()
      onClose()
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Import failed')
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      data-testid="wf-import-dialog"
      open={open}
      title="Import Workflow"
      onOpenChange={o => {
        if (!o) {
          reset()
          onClose()
        }
      }}
      footer={
        <>
          <Button data-testid="wf-import-validate-btn" variant="outline" loading={validating} onClick={handleValidate}>
            Validate
          </Button>
          <Button data-testid="wf-import-submit-btn" loading={submitting} onClick={handleImport}>
            Import
          </Button>
        </>
      }
    >
      <Space direction="vertical" className="w-full" size="middle">
        <Upload
          data-testid="wf-import-upload"
          label="Drop a workflow bundle or workflow.yaml"
          multiple={false}
          onFiles={fs => {
            setFiles(fs.slice(-1))
            setValidation(null)
          }}
        >
          <p className="ant-upload-drag-icon">
            <Inbox />
          </p>
          <p className="ant-upload-text">
            Drop a workflow bundle (.tar.gz) or workflow.yaml here
          </p>
          {files[0] && (
            <Text className="text-xs">{files[0].name}</Text>
          )}
          <Text type="secondary" className="text-xs">
            Imported workflows are marked Dev (mocks honored, tests runnable).
          </Text>
        </Upload>

        {validation && (
          <Alert
            data-testid="wf-import-validation-alert"
            tone={validation.valid ? 'success' : 'error'}
            title={
              validation.valid
                ? `Valid workflow — ${validation.steps} steps, up to ${validation.est_max_calls} calls`
                : 'Validation failed'
            }
            description={
              validation.errors.length > 0 || validation.warnings.length > 0 ? (
                <div className="flex flex-col gap-1">
                  {validation.errors.map((e, i) => (
                    <Text key={`e${i}`} type="danger" className="text-xs">
                      {e.location ? `${e.location}: ` : ''}
                      {e.message}
                    </Text>
                  ))}
                  {validation.warnings.map((w, i) => (
                    <Text key={`w${i}`} type="warning" className="text-xs">
                      {w.location ? `${w.location}: ` : ''}
                      {w.message}
                    </Text>
                  ))}
                </div>
              ) : undefined
            }
          />
        )}
      </Space>
    </Dialog>
  )
}
