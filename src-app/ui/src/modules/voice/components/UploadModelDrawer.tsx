import { Upload as UploadIcon } from 'lucide-react'
import { useState } from 'react'
import { Permissions } from '@/api-client/types'
import {
  Button,
  Card,
  Input,
  message,
  Progress,
  Text,
  Upload,
} from '@/components/ui'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { formatBytes } from '@/utils/downloadUtils'

/**
 * Upload a whisper ggml model file (.bin / .gguf) as a new installed model.
 * Mirrors llm-provider's AddLocalLlmModelUploadDrawer (kit Upload + name field
 * + per-file/overall Progress + Cancel), single-file.
 */
export function UploadModelDrawer() {
  const { open } = Stores.VoiceUploadModelDrawer
  const { uploading, uploadProgress, overallUploadProgress, uploadError } =
    Stores.VoiceModelUpload
  const canManage = usePermission(Permissions.VoiceAdminManage)

  const [file, setFile] = useState<File | null>(null)
  const [name, setName] = useState('')

  const handleFiles = (files: File[]) => {
    const first = files[0]
    if (!first) return
    setFile(first)
    if (!name.trim()) {
      // Derive a default name from the filename (strip ggml- prefix + extension).
      const base = first.name
        .replace(/\.(bin|gguf)$/i, '')
        .replace(/^ggml-/i, '')
      setName(base)
    }
  }

  const reset = () => {
    setFile(null)
    setName('')
    Stores.VoiceModelUpload.clearUploadState()
  }

  const handleClose = () => {
    if (uploading) {
      message.warning(
        'Upload in progress — please cancel or wait for it to complete',
      )
      return
    }
    reset()
    Stores.VoiceUploadModelDrawer.closeUploadModelDrawer()
  }

  const handleSubmit = async () => {
    if (!file) {
      message.error('Select a model file to upload')
      return
    }
    if (!name.trim()) {
      message.error('Enter a model name')
      return
    }
    try {
      await Stores.VoiceModelUpload.uploadModel({ name: name.trim(), file })
      message.success('Model uploaded')
      reset()
      Stores.VoiceUploadModelDrawer.closeUploadModelDrawer()
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to upload model')
    }
  }

  return (
    <Drawer
      title="Upload model"
      open={open}
      onClose={handleClose}
      size={600}
      mask={{ closable: !uploading }}
      closable={!uploading}
      footer={[
        <Button
          key="cancel"
          variant="outline"
          onClick={handleClose}
          disabled={uploading}
          data-testid="voice-upload-drawer-cancel-btn"
        >
          {canManage ? 'Cancel' : 'Close'}
        </Button>,
        canManage && (
          <Button
            key="submit"
            loading={uploading}
            onClick={handleSubmit}
            disabled={uploading || !file || !name.trim()}
            data-testid="voice-upload-drawer-submit-btn"
          >
            {uploading ? 'Uploading…' : 'Upload'}
          </Button>
        ),
      ]}
    >
      <div className="flex flex-col gap-4">
        <div className="flex flex-col gap-1">
          <label className="text-sm font-medium" htmlFor="voice-upload-name">
            Model name{' '}
            <span className="text-destructive ms-0.5" aria-hidden>
              *
            </span>
          </label>
          <Input
            id="voice-upload-name"
            value={name}
            onChange={e => setName(e.target.value)}
            placeholder="e.g. base.en"
            disabled={uploading}
            data-testid="voice-upload-name"
            aria-label="Model name"
          />
        </div>

        <div className="flex flex-col gap-1">
          <label className="text-sm font-medium">
            Model file{' '}
            <span className="text-destructive ms-0.5" aria-hidden>
              *
            </span>
          </label>
          <Upload
            onFiles={handleFiles}
            data-testid="voice-upload-files"
            label="Select a model file"
            aria-label="Model file"
            disabled={uploading}
            accept=".bin,.gguf"
          >
            <p>
              <UploadIcon
                className="mx-auto size-8 text-muted-foreground"
                aria-hidden
              />
            </p>
            <p className="text-sm font-medium">
              Click or drag a ggml model file to select
            </p>
            <p className="text-xs text-muted-foreground">
              Accepts .bin or .gguf
            </p>
          </Upload>
          {file && (
            <Text
              type="secondary"
              className="text-xs"
              data-testid="voice-upload-selected-file"
            >
              {file.name} — {formatBytes(file.size)}
            </Text>
          )}
        </div>

        {uploadError && (
          <Text type="danger" data-testid="voice-upload-error">
            {uploadError}
          </Text>
        )}

        {uploading &&
          (uploadProgress.length > 0 || overallUploadProgress > 0) && (
            <Card
              title="Upload progress"
              size="sm"
              data-testid="voice-upload-progress-card"
              extra={
                <Button
                  variant="link"
                  size="default"
                  onClick={() => Stores.VoiceModelUpload.cancelUpload()}
                  className="text-destructive"
                  data-testid="voice-upload-cancel-btn"
                >
                  Cancel upload
                </Button>
              }
            >
              {overallUploadProgress > 0 && (
                <div className="mb-3">
                  <Text strong>Overall progress:</Text>
                  <Progress
                    value={Math.round(overallUploadProgress)}
                    tone="primary"
                    aria-label="Overall upload progress"
                    data-testid="voice-upload-overall-progress"
                  />
                </div>
              )}
              {uploadProgress.map((fp, index) => (
                <div key={index} className="mb-2">
                  <Text strong>{fp.filename}</Text>
                  <Progress
                    value={Math.round(fp.progress)}
                    tone={fp.status === 'error' ? 'error' : 'primary'}
                    aria-label={`Upload progress for ${fp.filename}`}
                    data-testid={`voice-upload-file-progress-${index}`}
                  />
                  {fp.size > 0 && (
                    <Text type="secondary" className="text-xs">
                      {formatBytes(Math.round((fp.progress * fp.size) / 100))}{' '}
                      of {formatBytes(fp.size)} uploaded
                    </Text>
                  )}
                </div>
              ))}
            </Card>
          )}
      </div>
    </Drawer>
  )
}
