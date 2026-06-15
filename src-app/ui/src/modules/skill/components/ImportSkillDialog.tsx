import { InboxOutlined } from '@ant-design/icons'
import type { UploadFile } from 'antd'
import { Alert, App, Button, Modal, Space, Typography, Upload } from 'antd'
import { useState } from 'react'
import type { ValidateSkillResponse } from '@/api-client/types'
import { Stores } from '@/core/stores'

const { Dragger } = Upload
const { Text } = Typography

interface ImportSkillDialogProps {
  open: boolean
  onClose: () => void
  /** When true, import as a system-scope skill (admin). */
  system?: boolean
}

/**
 * Import a skill from a local bundle (tar.gz of the source dir). The
 * bundle's SKILL.md is validated server-side; an inline /validate
 * result is shown when the user supplies the SKILL.md text. Re-import
 * overwrites without version bumping (is_dev=true).
 */
export function ImportSkillDialog({
  open,
  onClose,
  system,
}: ImportSkillDialogProps) {
  const { message } = App.useApp()
  const [fileList, setFileList] = useState<UploadFile[]>([])
  const [validation, setValidation] = useState<ValidateSkillResponse | null>(
    null,
  )
  const [submitting, setSubmitting] = useState(false)
  const [validating, setValidating] = useState(false)

  const reset = () => {
    setFileList([])
    setValidation(null)
    setSubmitting(false)
    setValidating(false)
  }

  const handleValidate = async () => {
    const file = fileList[0]?.originFileObj
    if (!file) {
      message.warning('Select a SKILL.md or bundle to validate')
      return
    }
    // /validate takes the SKILL.md text. If the user dropped a .md file
    // we can read it directly; for a tar.gz we defer to the import path.
    if (!file.name.endsWith('.md')) {
      message.info(
        'Validation reads SKILL.md text; drop a SKILL.md to preview validation, or import the bundle directly.',
      )
      return
    }
    setValidating(true)
    try {
      const text = await file.text()
      const result = await Stores.Skill.validateSkill(text)
      setValidation(result)
    } catch {
      message.error('Validation request failed')
    } finally {
      setValidating(false)
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
        await Stores.SystemSkill.importSystemSkill(form)
      } else {
        await Stores.Skill.importSkill(form)
      }
      message.success('Skill imported')
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
      title="Import Skill"
      onCancel={() => {
        reset()
        onClose()
      }}
      footer={[
        <Button key="validate" loading={validating} onClick={handleValidate}>
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
            Drop a skill bundle (.tar.gz) or SKILL.md here
          </p>
          <Text type="secondary" className="text-xs">
            Imported skills are marked Dev (mocks honored, no version bumping).
          </Text>
        </Dragger>

        {validation && (
          <Alert
            type={validation.valid ? 'success' : 'error'}
            showIcon
            title={validation.valid ? 'Valid skill' : 'Validation failed'}
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
