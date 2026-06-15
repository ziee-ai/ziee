import { InboxOutlined } from '@ant-design/icons'
import type { UploadFile } from 'antd'
import { Alert, App, Button, Modal, Space, Typography, Upload } from 'antd'
import { useState } from 'react'
import type { ValidateWorkflowResponse } from '@/api-client/types'
import { Stores } from '@/core/stores'

const { Dragger } = Upload
const { Text } = Typography

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
  const { message } = App.useApp()
  const [fileList, setFileList] = useState<UploadFile[]>([])
  const [validation, setValidation] = useState<ValidateWorkflowResponse | null>(
    null,
  )
  const [submitting, setSubmitting] = useState(false)

  const reset = () => {
    setFileList([])
    setValidation(null)
    setSubmitting(false)
  }

  const handleValidate = async () => {
    const file = fileList[0]?.originFileObj
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
    const text = await file.text()
    try {
      setValidation(await Stores.Workflow.validateWorkflow(text))
    } catch {
      message.error('Validation request failed')
    }
  }

  const handleImport = async () => {
    const file = fileList[0]?.originFileObj
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
        await Stores.SystemWorkflow.importSystemWorkflow(form)
      } else {
        await Stores.Workflow.importWorkflow(form)
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
    <Modal
      open={open}
      title="Import Workflow"
      onCancel={() => {
        reset()
        onClose()
      }}
      footer={[
        <Button key="validate" onClick={handleValidate}>
          Validate
        </Button>,
        <Button
          key="import"
          type="primary"
          loading={submitting}
          onClick={handleImport}
        >
          Import
        </Button>,
      ]}
    >
      <Space vertical className="w-full" size="middle">
        <Dragger
          fileList={fileList}
          beforeUpload={() => false}
          maxCount={1}
          onChange={info => {
            setFileList(info.fileList.slice(-1))
            setValidation(null)
          }}
        >
          <p className="ant-upload-drag-icon">
            <InboxOutlined />
          </p>
          <p className="ant-upload-text">
            Drop a workflow bundle (.tar.gz) or workflow.yaml here
          </p>
          <Text type="secondary" className="text-xs">
            Imported workflows are marked Dev (mocks honored, tests runnable).
          </Text>
        </Dragger>

        {validation && (
          <Alert
            type={validation.valid ? 'success' : 'error'}
            showIcon
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
    </Modal>
  )
}
