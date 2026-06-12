import { useState, useEffect } from 'react'
import {
  App,
  AutoComplete,
  Button,
  Card,
  Form,
  Input,
  Progress,
  Select,
  Typography,
} from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import {} from '@/modules/llm-provider/stores'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { LocalLlmModelCommonFields } from '@/modules/llm-provider/components/llm-models/shared/LocalLlmModelCommonFields'
import {
  Permissions,
  type FileFormat,
  type RepositoryFileListResponse,
} from '@/api-client/types'

const { Text } = Typography

// Last path segment, e.g. "sub/model.safetensors" -> "model.safetensors".
const baseName = (p: string): string => p.split('/').pop() || p

const humanSize = (n: number): string => {
  if (!n || n <= 0) return ''
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let v = n
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(v >= 10 || i === 0 ? 0 : 1)} ${units[i]}`
}

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

// Prefix before a `-NNNNN-of-MMMMM` / `_NNNNN_of_MMMMM_` shard infix
// (mirrors the backend model_files::shard_prefix), or null when not sharded.
const shardPrefix = (name: string): string | null => {
  // Match the backend model_files::shard_prefix grammar exactly: a
  // CONSISTENT separator, `-NNN-of-NNN` or `_NNN_of_NNN` (no mixing).
  const bn = name.split('/').pop() || name
  const m = bn.match(/^(.+?)-\d+-of-\d+/i) || bn.match(/^(.+?)_\d+_of_\d+/i)
  return m ? m[1] : null
}

export function AddLocalLlmModelDownloadDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)
  const [detecting, setDetecting] = useState(false)
  const [detected, setDetected] = useState<RepositoryFileListResponse | null>(
    null,
  )

  const { open: addMode, providerId } = Stores.AddLocalLlmModelDownloadDrawer
  const { open: viewMode, downloadId } = Stores.ViewDownloadDrawer
  const { downloads } = Stores.LlmModelDownload
  // Read repositories from the canonical LlmRepository store (whose
  // __init__ hits /api/llm-repositories once and caches; filter here
  // because the drawer only offers enabled repos as download targets).
  // Previously inlined an ApiClient.LlmRepository.list call into a
  // useState — bypassed the store cache and missed any subsequent
  // create/update/delete events.
  const repositories = Stores.LlmRepository.repositories.filter(
    r => r.enabled,
  )
  const loadingRepositories = Stores.LlmRepository.loading
  const canCreate = usePermission(Permissions.LlmModelsCreate)
  const canCancelDownload = usePermission(Permissions.LlmModelsDownloadsCancel)

  const open = viewMode || addMode

  // Get selected repository from form
  const selectedRepository = Form.useWatch('repository_id', form)
  const watchedPath = Form.useWatch('repository_path', form)
  const watchedBranch = Form.useWatch('repository_branch', form)

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
      form.setFieldsValue({ main_filename: '' })
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
    form.resetFields()
  }

  // Detect the model files in the selected repository so the user can pick
  // the main file (GGUF quant) or have the safetensors set auto-selected,
  // instead of typing the filename blind.
  const handleDetectFiles = async () => {
    const repositoryId = form.getFieldValue('repository_id')
    const path = (form.getFieldValue('repository_path') || '').trim()
    const branch = form.getFieldValue('repository_branch') || 'main'
    if (!repositoryId || !path) {
      message.error('Select a repository and enter a repository path first')
      return
    }
    try {
      setDetecting(true)
      const res = await Stores.LlmModelDownload.listRepositoryFiles(
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
      form.setFieldsValue({
        ...(res.suggested_main_filename
          ? { main_filename: res.suggested_main_filename }
          : {}),
        ...(fmt ? { file_format: fmt } : {}),
      })
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
  const mainFileOptions = isGguf
    ? // One entry per quant. A sharded GGUF set (`*-00001-of-00003.gguf`)
      // downloads as a group, so collapse it to a single option (the first
      // shard) instead of listing every shard.
      (() => {
        const seen = new Set<string>()
        const opts: { value: string; label: string }[] = []
        // Sort by path so a sharded set collapses to its first shard
        // (`-00001-of-*`) deterministically rather than upstream-list order.
        const ggufFiles = weightFiles
          .filter(f => f.path.toLowerCase().endsWith('.gguf'))
          .sort((a, b) => a.path.localeCompare(b.path))
        for (const f of ggufFiles) {
          const key = shardPrefix(f.path) ?? baseName(f.path)
          if (seen.has(key)) continue
          seen.add(key)
          opts.push({
            value: baseName(f.path),
            label: `${baseName(f.path)}  ${humanSize(f.size_bytes)}`.trim(),
          })
        }
        return opts
      })()
    : detected?.suggested_main_filename
      ? [{ value: detected.suggested_main_filename, label: detected.suggested_main_filename }]
      : []
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

  const handleSubmit = async () => {
    try {
      setLoading(true)
      Stores.LlmProvider.clearLlmProviderStoreError()
      const values = await form.validateFields()

      // Auto-generate model ID from display name
      const modelId = generateModelId(values.display_name || 'model')

      if (!values.repository_id) {
        message.error('Repository is required')
        return
      }

      if (!values.repository_path) {
        message.error('Repository path is required')
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
        await Stores.LlmModelDownload.downloadLlmModelFromRepository(
          {
            provider_id: providerId!,
            repository_id: values.repository_id,
            repository_path: values.repository_path,
            main_filename: values.main_filename,
            repository_branch: values.repository_branch || 'main',
            name: modelId,
            display_name: values.display_name,
            description: values.description,
            // Format follows the chosen file's extension (falling back to the
            // dropdown only when the extension is unrecognized, e.g. an index).
            file_format:
              formatForFilename(values.main_filename) ??
              (values.file_format as FileFormat),
            capabilities: values.capabilities || {},
            parameters: values.parameters || {},
            engine_type: values.engine_type || 'mistralrs',
            engine_settings: values.engine_settings || {},
          },
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
        form.setFieldsValue({
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
        form.setFieldsValue({
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
  }, [open, viewMode, viewDownload, form])

  return (
    <Drawer
      title={viewMode ? 'View Download Details' : 'Download from Repository'}
      open={open}
      onClose={handleCloseModal}
      footer={
        viewMode
          ? [
              <Button key="close" onClick={handleCloseModal}>
                Close
              </Button>,
              canCancelDownload &&
                viewDownload &&
                (viewDownload.status === 'downloading' ||
                  viewDownload.status === 'pending') && (
                  <Button
                    key="cancel-download"
                    danger
                    onClick={async () => {
                      try {
                        await Stores.LlmModelDownload.cancelLlmModelDownload(
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
              <Button key="cancel" onClick={handleCancel}>
                {canCreate ? 'Cancel' : 'Close'}
              </Button>,
              canCreate && (
                <Button
                  key="submit"
                  type="primary"
                  loading={loading}
                  onClick={handleSubmit}
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
          <Card title="Download Progress" style={{ marginBottom: 16 }}>
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
                  percent={Math.round(
                    ((viewDownload.progress_data?.current || 0) /
                      (viewDownload.progress_data?.total || 1)) *
                      100,
                  )}
                  status={
                    viewDownload.status === 'downloading'
                      ? 'active'
                      : viewDownload.status === 'completed'
                        ? 'success'
                        : viewDownload.status === 'failed'
                          ? 'exception'
                          : 'normal'
                  }
                  format={percent => `${percent}%`}
                />
                {viewDownload.progress_data && (
                  <Text type="secondary" style={{ fontSize: '12px' }}>
                    {viewDownload.progress_data.message || ''}
                  </Text>
                )}
                {viewDownload.progress_data?.speed_bps && (
                  <div style={{ marginTop: 8 }}>
                    <Text type="secondary" style={{ fontSize: '12px' }}>
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
          layout="vertical"
          disabled={viewMode}
          initialValues={{
            file_format: 'safetensors',
            main_filename: '',
            repository_branch: 'main',
          }}
        >
          <LocalLlmModelCommonFields />

          <Form.Item
            name="repository_id"
            label="Repository"
            rules={[
              {
                required: true,
                message: 'Repository is required',
              },
            ]}
          >
            <Select
              placeholder="Select repository"
              loading={loadingRepositories}
              showSearch={{ optionFilterProp: 'children' }}
              options={repositories.map(repo => ({
                value: repo.id,
                label: `${repo.name} (${repo.url})`,
              }))}
            />
          </Form.Item>

          <Form.Item
            name="repository_path"
            label="Repository Path"
            rules={[
              {
                required: true,
                message: 'Repository path is required',
              },
            ]}
          >
            <Input
              placeholder="microsoft/DialoGPT-medium"
              prefix={
                selectedRepository
                  ? repositories.find(r => r.id === selectedRepository)?.url ||
                    'Repository'
                  : 'Repository'
              }
            />
          </Form.Item>

          {!viewMode && (
            <Form.Item>
              <Button onClick={handleDetectFiles} loading={detecting}>
                Detect files
              </Button>
            </Form.Item>
          )}

          <Form.Item
            name="main_filename"
            label="Main Filename"
            rules={[
              {
                required: true,
                message: 'Main filename is required',
              },
            ]}
            extra={viewMode ? undefined : detectHelp}
          >
            <AutoComplete
              placeholder="model.safetensors"
              options={mainFileOptions}
            />
          </Form.Item>

          <Form.Item name="repository_branch" label="Branch">
            <Input placeholder="main" />
          </Form.Item>

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
