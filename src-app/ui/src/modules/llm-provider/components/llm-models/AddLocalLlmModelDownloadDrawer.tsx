import { useState, useEffect } from 'react'
import {
  App,
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
import { Permissions, type FileFormat } from '@/api-client/types'

const { Text } = Typography

export function AddLocalLlmModelDownloadDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

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
    form.resetFields()
  }

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
            file_format: values.file_format as FileFormat,
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
          repository_path: 'meta-llama/Llama-3.1-8B-Instruct',
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
      maskClosable={false}
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

          <Form.Item
            name="main_filename"
            label="Main Filename"
            rules={[
              {
                required: true,
                message: 'Main filename is required',
              },
            ]}
          >
            <Input placeholder="model.safetensors" />
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
