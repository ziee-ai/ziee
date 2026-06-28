import { Inbox } from 'lucide-react'
import { Alert, Button, Dialog, Space, Text, Upload, message } from '@/components/ui'
import { useState } from 'react'
import type { ValidateSkillResponse } from '@/api-client/types'
import { Stores } from '@/core/stores'

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
  const [file, setFile] = useState<File | null>(null)
  const [validation, setValidation] = useState<ValidateSkillResponse | null>(
    null,
  )
  const [submitting, setSubmitting] = useState(false)
  const [validating, setValidating] = useState(false)

  const reset = () => {
    setFile(null)
    setValidation(null)
    setSubmitting(false)
    setValidating(false)
  }

  const handleValidate = async () => {
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
    <Dialog
      open={open}
      data-testid="skill-import-dialog"
      title="Import Skill"
      onOpenChange={o => {
        if (!o) {
          reset()
          onClose()
        }
      }}
      footer={
        <>
          <Button variant="outline" loading={validating} data-testid="skill-import-validate-button" onClick={handleValidate}>
            Validate
          </Button>
          <Button loading={submitting} data-testid="skill-import-submit-button" onClick={handleImport}>
            Import
          </Button>
        </>
      }
    >
      <Space direction="vertical" className="w-full" size="middle">
        <Upload
          label="Skill bundle"
          data-testid="skill-import-upload"
          onFiles={files => {
            setFile(files[0] ?? null)
            setValidation(null)
          }}
        >
          <Inbox />
          <span className="text-sm">
            Drop a skill bundle (.tar.gz) or SKILL.md here
          </span>
          <Text type="secondary" className="text-xs">
            Imported skills are marked Dev (mocks honored, no version bumping).
          </Text>
          {file && <Text className="text-xs">{file.name}</Text>}
        </Upload>

        {validation && (
          <Alert
            data-testid="skill-import-validation-alert"
            tone={validation.valid ? 'success' : 'error'}
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
    </Dialog>
  )
}
