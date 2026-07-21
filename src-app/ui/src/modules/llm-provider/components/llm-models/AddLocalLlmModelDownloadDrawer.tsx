import { useState, useEffect } from 'react'
import {
  Button,
  Card,
  Form,
  FormField,
  Input,
  Progress,
  Select,
  Text,
  useForm,
  message,
} from '@ziee/kit'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import {} from '@/modules/llm-provider/stores'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { LocalLlmModelCommonFields } from '@/modules/llm-provider/components/llm-models/shared/LocalLlmModelCommonFields'
import { type FileFormat, type EngineType, type RepositoryFileListResponse } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { LlmModelDownload } from '@/modules/llm-provider/stores/llmModelDownload'
import { LlmProvider } from '@/modules/llm-provider/stores/llmProvider'
import { LlmRepository as LlmRepositoryStore } from '@/modules/llm-repository/stores/llmRepository'

const formatForShape = (shape: string): FileFormat | undefined => {
  if (shape === 'gguf') return 'gguf'
  if (shape === 'safetensors') return 'safetensors'
  if (shape === 'pickle') return 'pytorch'
  return undefined
}

// Derive the file_format from the chosen main filename's extension. The
// backend stores file_format verbatim (never recomputes it from the file),
// so making it follow the actual file prevents a stale detection value
// (e.g. gguf) from being submitted for a manually-typed .safetensors file.
const formatForFilename = (name?: string): FileFormat | undefined => {
  const n = (name || '').toLowerCase()
  if (n.endsWith('.gguf')) return 'gguf'
  if (n.endsWith('.safetensors')) return 'safetensors'
  if (n.endsWith('.bin') || n.endsWith('.pt') || n.endsWith('.pth'))
    return 'pytorch'
  return undefined
}

// Human label for the detected source (the raw enum is a lowercase id).
const SOURCE_LABEL: Record<string, string> = {
  huggingface: 'Hugging Face',
  github: 'GitHub',
  unknown: '',
}

export function AddLocalLlmModelDownloadDrawer() {
  const [loading, setLoading] = useState(false)
  const [detecting, setDetecting] = useState(false)
  const [detected, setDetected] = useState<RepositoryFileListResponse | null>(
    null,
  )
  const form = useForm<Record<string, unknown>>({
    defaultValues: {
      file_format: 'safetensors',
      main_filename: '',
      repository_branch: 'main',
    },
  })

  const { open: addMode, providerId } = Stores.AddLocalLlmModelDownloadDrawer
  const { open: viewMode, downloadId } = Stores.ViewDownloadDrawer
  const { downloads } = LlmModelDownload
  // Read repositories from the canonical LlmRepository store (whose
  // __init__ hits /api/llm-repositories once and caches; filter here
  // because the drawer only offers enabled repos as download targets).
  // Previously inlined an ApiClient.LlmRepository.list call into a
  // useState — bypassed the store cache and missed any subsequent
  // create/update/delete events.
  const repositories = LlmRepositoryStore.repositories.filter(
    r => r.enabled,
  )
  const loadingRepositories = LlmRepositoryStore.loading
  const canCreate = usePermission(Permissions.LlmModelsCreate)
  const canCancelDownload = usePermission(Permissions.LlmModelsDownloadsCancel)

  const open = viewMode || addMode

  // Get selected repository from form
  const selectedRepository = form.watch('repository_id') as string | undefined
  const watchedPath = form.watch('repository_path')
  const watchedBranch = form.watch('repository_branch')

  // Invalidate a prior detection when the target repo / path / branch
  // changes, so the picker + the auto-filled main filename / format never
  // reflect a stale repository. Gated on `detected` so the open-time form
  // populate (which also mutates these watched fields) isn't clobbered.
  useEffect(() => {
    if (detected) {
      setDetected(null)
      // Clear only the now-stale auto-filled filename. Leave file_format
      // untouched — resetting it to 'safetensors' would wrongly clobber a
      // gguf/pytorch choice; a re-detect sets it correctly anyway.
      form.setValue('main_filename', '')
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedRepository, watchedPath, watchedBranch])

  // Get download instance from store
  const viewDownload = downloads.find(d => d.id === downloadId)

  // Function to generate a unique model ID from display name
  const generateModelId = (displayName: string): string => {
    const baseId = displayName
      .toLowerCase()
      .replace(/[^a-z0-9\s-]/g, '')
      .replace(/\s+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '')
      .substring(0, 50)

    const timestamp = Date.now().toString(36)
    return `${baseId}-${timestamp}`
  }

  // Helper function to close the modal
  const handleCloseModal = () => {
    Stores.AddLocalLlmModelDownloadDrawer.closeAddLocalLlmModelDownloadDrawer()
    Stores.ViewDownloadDrawer.closeViewDownloadDrawer()
    setLoading(false)
    setDetected(null)
    form.reset()
  }

  // Detect the model files in the selected repository so the user can pick
  // the main file (GGUF quant) or have the safetensors set auto-selected,
  // instead of typing the filename blind.
  const handleDetectFiles = async () => {
    const repositoryId = form.getValues('repository_id') as string | undefined
    const path = ((form.getValues('repository_path') as string) || '').trim()
    const branch = (form.getValues('repository_branch') as string) || 'main'
    if (!repositoryId || !path) {
      message.error('Select a repository and enter a repository path first')
      return
    }
    try {
      setDetecting(true)
      const res = await LlmModelDownload.listRepositoryFiles(
        repositoryId,
        path,
        branch,
      )
      if (res.source === 'unknown') {
        // Don't store an unknown detection: there's nothing to pick, and
        // keeping it null leaves the user's manually-typed filename intact
        // (the invalidation effect is gated on `detected`).
        setDetected(null)
        message.info(
          'Auto-detect supports Hugging Face and GitHub. Enter the main filename manually.',
        )
        return
      }
      if (res.files.length === 0) {
        setDetected(null)
        message.warning('No files found for that repository path / branch.')
        return
      }
      setDetected(res)
      // Pre-fill the main filename + file format from the detection.
      const fmt = formatForShape(res.shape)
      if (res.suggested_main_filename) {
        form.setValue('main_filename', res.suggested_main_filename)
      }
      if (fmt) {
        form.setValue('file_format', fmt)
      }
      const sourceLabel = SOURCE_LABEL[res.source]
      message.success(
        `Detected ${res.files.length}${res.truncated ? '+' : ''} files${
          sourceLabel ? ` from ${sourceLabel}` : ''
        }`,
      )
    } catch (error: any) {
      console.error('Failed to detect repository files:', error)
      message.error(
        `Failed to detect files: ${error?.message || 'request failed'}`,
      )
    } finally {
      setDetecting(false)
    }
  }

  // Options + help text for the main-filename picker derived from detection.
  const isGguf = detected?.shape === 'gguf'
  const weightFiles = (detected?.files ?? []).filter(
    f => f.file_role === 'weight',
  )
  // Count only the weights of the detected shape (a mixed repo may also carry
  // e.g. a pytorch_model.bin that isn't part of the safetensors set).
  const shapeFormat = detected ? formatForShape(detected.shape) : undefined
  const shapeWeightCount = weightFiles.filter(
    f => f.file_format === shapeFormat,
  ).length
  // Truncation only affects the *picker options* (GGUF), where the visible
  // list drives the choice. For safetensors/pickle the backend grabs the
  // whole weight set from the clone regardless of the listing, so no warning.
  const truncatedNote =
    detected?.truncated && isGguf
      ? ' The quant list was truncated — some quantizations may not be shown.'
      : ''
  const detectHelp =
    (!detected
      ? 'Click "Detect files" to list the model files in the repository.'
      : detected.source === 'unknown'
        ? 'Auto-detect supports Hugging Face and GitHub repositories — enter the main filename manually.'
        : isGguf
          ? 'Pick a GGUF quantization.'
          : detected.shape === 'safetensors' || detected.shape === 'pickle'
            ? `Detected a ${shapeWeightCount}-file ${detected.shape} model — the full weight set downloads automatically.`
            : 'Enter the main weight filename.') + truncatedNote

  const onValid = async (values: Record<string, unknown>) => {
    try {
      setLoading(true)
      LlmProvider.clearLlmProviderStoreError()

      // Auto-generate model ID from display name
      const modelId = generateModelId((values.display_name as string) || 'model')

      // Required-field validation (this form has no zod resolver). Surface
      // errors as inline FieldErrors (role="alert") rather than toasts.
      let hasFieldError = false
      if (!values.repository_id) {
        form.setError('repository_id', { message: 'Repository is required' })
        hasFieldError = true
      }
      if (!(values.repository_path as string | undefined)?.trim()) {
        form.setError('repository_path', { message: 'Repository path is required' })
        hasFieldError = true
      }
      if (!(values.display_name as string | undefined)?.trim()) {
        form.setError('display_name', { message: 'Display name is required' })
        hasFieldError = true
      }
      if (!(values.main_filename as string | undefined)?.trim()) {
        form.setError('main_filename', { message: 'Main filename is required' })
        hasFieldError = true
      }
      if (hasFieldError) {
        return
      }

      // Get the selected repository details
      const selectedRepo = repositories.find(
        repo => repo.id === values.repository_id,
      )
      if (!selectedRepo) {
        message.error('Repository not found')
        return
      }

      // Check for duplicate downloads
      const isAnotherDownloadInProgress = downloads.some(
        download =>
          download.provider_id === providerId &&
          download.repository_id === values.repository_id &&
          download.request_data.repository_path === values.repository_path &&
          (download.status === 'downloading' || download.status === 'pending'),
      )

      if (isAnotherDownloadInProgress) {
        message.error(
          'Another download with the same repository is already in progress. Please wait for it to complete.',
        )
        return
      }

      // Call the repository download API through store
      try {
        await LlmModelDownload.downloadLlmModelFromRepository(
          {
            provider_id: providerId!,
            repository_id: values.repository_id as string,
            repository_path: values.repository_path as string,
            main_filename: values.main_filename as string,
            repository_branch: (values.repository_branch as string) || 'main',
            name: modelId,
            display_name: values.display_name as string,
            description: values.description as string,
            // Format follows the chosen file's extension (falling back to the
            // dropdown only when the extension is unrecognized, e.g. an index).
            file_format:
              formatForFilename(values.main_filename as string) ??
              (values.file_format as FileFormat),
            capabilities: (values.capabilities as Record<string, unknown>) || {},
            parameters: (values.parameters as Record<string, unknown>) || {},
            engine_type: ((values.engine_type as string) || 'mistralrs') as EngineType,
            engine_settings: (values.engine_settings as Record<string, unknown>) || {},
          },
          // `onStart` fires as soon as the download is registered: it switches
          // this shared drawer from add-mode to View-Download-Details mode
          // (`open = viewMode || addMode`), so the editable form (its submit
          // button) is replaced by the read-only view.
          Stores.ViewDownloadDrawer.openViewDownloadDrawer,
        )

        message.success('Download started successfully')
      } catch (error) {
        console.error('Failed to start download:', error)
        message.error('Failed to start download')
      }
    } catch (error) {
      console.error('Failed to create model:', error)
      message.error('Failed to create model')
    } finally {
      setLoading(false)
    }
  }

  const handleCancel = () => {
    handleCloseModal()
  }

  // Pre-fill form when modal opens (repositories are now read
  // reactively from the LlmRepository store at top level).
  useEffect(() => {
    if (open) {
      if (viewDownload) {
        // In view mode, populate form with download data from request_data
        const requestData = viewDownload.request_data
        form.reset({
          display_name: requestData.display_name,
          description: requestData.description || '',
          file_format: requestData.file_format,
          repository_id: viewDownload.repository_id,
          repository_path: requestData.repository_path,
          main_filename: requestData.main_filename,
          repository_branch: requestData.revision || 'main',
          capabilities: requestData.capabilities || {},
          parameters: requestData.parameters || {},
          engine_type: requestData.engine_type || 'mistralrs',
          engine_settings: requestData.engine_settings || {},
        })
      } else if (!viewMode) {
        // In add mode, set default values
        form.reset({
          display_name: 'TinyLlama Chat Model',
          description:
            'Small 1.1B parameter chat model for quick testing (~637MB)',
          file_format: 'safetensors',
          // Ungated repo that matches the TinyLlama display name/description
          // above (the prior meta-llama default was gated + inconsistent).
          repository_path: 'TinyLlama/TinyLlama-1.1B-Chat-v1.0',
          main_filename: 'model.safetensors',
          repository_branch: 'main',
        })
      }
    }
  }, [open, viewMode, viewDownload])

  return (
    <Drawer
      title={viewMode ? 'View Download Details' : 'Download from Repository'}
      open={open}
      onClose={handleCloseModal}
      footer={
        viewMode
          ? [
              <Button key="close" variant="outline" onClick={handleCloseModal} data-testid="llm-download-drawer-close-btn">
                Close
              </Button>,
              canCancelDownload &&
                viewDownload &&
                (viewDownload.status === 'downloading' ||
                  viewDownload.status === 'pending') && (
                  <Button
                    key="cancel-download"
                    variant="destructive"
                    data-testid="llm-download-drawer-cancel-download-btn"
                    onClick={async () => {
                      try {
                        await LlmModelDownload.cancelLlmModelDownload(
                          viewDownload.id,
                        )
                        message.success('Download cancelled successfully')
                      } catch (error: any) {
                        console.error('Failed to cancel download:', error)
                        message.error(
                          `Failed to cancel download: ${error.message}`,
                        )
                      }
                    }}
                  >
                    Cancel Download
                  </Button>
                ),
            ].filter(Boolean)
          : [
              <Button key="cancel" variant="outline" onClick={handleCancel} data-testid="llm-download-drawer-cancel-btn">
                {canCreate ? 'Cancel' : 'Close'}
              </Button>,
              canCreate && (
                <Button
                  key="submit"
                  loading={loading}
                  onClick={() => form.handleSubmit(onValid)()}
                  data-testid="llm-download-drawer-submit-btn"
                >
                  Download
                </Button>
              ),
            ]
      }
      size={600}
      mask={{ closable: false }}
    >
      <div>
        {viewDownload && (
          <Card title="Download Progress" className="mb-4" data-testid="llm-download-progress-card">
            {viewDownload.status === 'failed' && viewDownload.error_message ? (
              <Text type="danger">{viewDownload.error_message}</Text>
            ) : (
              <>
                {viewDownload.progress_data && (
                  <Text>
                    {viewDownload.progress_data?.phase || viewDownload.status}
                  </Text>
                )}
                <Progress
                  value={Math.round(
                    ((viewDownload.progress_data?.current || 0) /
                      (viewDownload.progress_data?.total || 1)) *
                      100,
                  )}
                  tone={
                    viewDownload.status === 'downloading'
                      ? 'primary'
                      : viewDownload.status === 'completed'
                        ? 'success'
                        : viewDownload.status === 'failed'
                          ? 'error'
                          : 'primary'
                  }
                  format={percent => `${percent}%`}
                  aria-label="Download progress"
                  data-testid="llm-download-detail-progress"
                />
                {viewDownload.progress_data && (
                  <Text type="secondary" className="text-xs">
                    {viewDownload.progress_data.message || ''}
                  </Text>
                )}
                {viewDownload.progress_data?.speed_bps && (
                  <div className="mt-2">
                    <Text type="secondary" className="text-xs">
                      Speed:{' '}
                      {Math.round(
                        (viewDownload.progress_data.speed_bps / 1024 / 1024) *
                          10,
                      ) / 10}{' '}
                      MB/s
                      {viewDownload.progress_data.eta_seconds && (
                        <>
                          {' '}
                          • ETA:{' '}
                          {Math.round(
                            viewDownload.progress_data.eta_seconds / 60,
                          )}{' '}
                          minutes
                        </>
                      )}
                    </Text>
                  </div>
                )}
              </>
            )}
          </Card>
        )}

        <Form
          name="llm-model-download"
          form={form}
          onSubmit={onValid}
          layout="vertical"
          disabled={viewMode}
          data-testid="llm-model-download-form"
        >
          <LocalLlmModelCommonFields />

          <FormField
            name="repository_id"
            label="Repository"
            required
          >
            <Select
              placeholder="Select repository"
              data-testid="llm-download-repository-select"
              loading={loadingRepositories}
              options={repositories.map(repo => ({
                value: repo.id,
                label: `${repo.name} (${repo.url})`,
              }))}
            />
          </FormField>

          <FormField
            name="repository_path"
            label="Repository Path"
            required
          >
            <Input
              placeholder="microsoft/DialoGPT-medium"
              data-testid="llm-download-repository-path-input"
              prefix={
                selectedRepository
                  ? repositories.find(r => r.id === selectedRepository)?.url ||
                    'Repository'
                  : 'Repository'
              }
            />
          </FormField>

          {!viewMode && (
            <div className="mb-4">
              <Button variant="outline" onClick={handleDetectFiles} loading={detecting} data-testid="llm-download-detect-files-btn">
                Detect files
              </Button>
            </div>
          )}

          <FormField
            name="main_filename"
            label="Main Filename"
            required
            description={viewMode ? undefined : detectHelp}
          >
            <Input placeholder="model.safetensors" data-testid="llm-download-main-filename-input" />
          </FormField>

          <FormField name="repository_branch" label="Branch">
            <Input placeholder="main" data-testid="llm-download-branch-input" />
          </FormField>

          {/*
           * Clear-cache toggle removed — the backend's
           * DownloadFromRepositoryRequest dropped the `clear_cache`
           * field (07-llm-model F-17): it was a stale debug flag
           * reachable in production that let any caller wipe the
           * server-side git cache. Useful only for testing; if you
           * need to force a fresh download, restart the server's
           * cache directory manually.
           */}
        </Form>
      </div>
    </Drawer>
  )
}
