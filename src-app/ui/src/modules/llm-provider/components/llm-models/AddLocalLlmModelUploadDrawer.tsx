import { UploadOutlined } from '@ant-design/icons'
import {
  App,
  Button,
  Card,
  Form,
  List,
  Progress,
  Select,
  Tag,
  Typography,
  Upload,
} from 'antd'
import { Drawer } from '@/components/common/Drawer'
import { useEffect, useState } from 'react'
import { LOCAL_FILE_TYPE_OPTIONS } from '../../constants'
import {
  cancelUpload,
  clearUploadError,
  uploadLocalModel,
  useAddLocalLlmModelUploadDrawerStore,
  useUploadStore,
} from '../../store'
import { formatBytes } from '@/utils/downloadUtils'
import { LocalLlmModelCommonFields } from './shared/LocalLlmModelCommonFields'

const { Text } = Typography

/**
 * File with metadata for display
 */
interface FilteredFile {
  file: File
  purpose: string
  required: boolean
}

export function AddLocalLlmModelUploadDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)
  const [selectedFiles, setSelectedFiles] = useState<File[]>([])
  const [filteredFiles, setFilteredFiles] = useState<FilteredFile[]>([])

  const { uploading, uploadProgress, overallUploadProgress } = useUploadStore()
  const { open, providerId } = useAddLocalLlmModelUploadDrawerStore()

  /**
   * Generate a unique model ID from display name
   */
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

  /**
   * Validate model files for required components
   */
  const validateModelFiles = (
    files: File[],
    fileFormat: string,
  ): { isValid: boolean; errors: string[]; warnings: string[] } => {
    const errors: string[] = []
    const warnings: string[] = []

    // Get expected extensions based on file format
    const expectedExtensions =
      LOCAL_FILE_TYPE_OPTIONS.find(option => option.value === fileFormat)
        ?.extensions || []

    // Check for main model files
    const hasMainFile = files.some(file =>
      expectedExtensions.some(ext => file.name.endsWith(ext)),
    )

    if (!hasMainFile) {
      errors.push(
        `No main model file found with expected extensions: ${expectedExtensions.join(', ')}`,
      )
    }

    // Check for potentially useful files
    const hasTokenizerFiles = files.some(
      file =>
        file.name.includes('tokenizer') ||
        file.name.endsWith('.json') ||
        file.name.endsWith('.txt'),
    )

    if (!hasTokenizerFiles) {
      warnings.push(
        'No tokenizer or configuration files detected. Model may not work properly.',
      )
    }

    return {
      isValid: errors.length === 0,
      errors,
      warnings,
    }
  }

  /**
   * Filter files based on the selected format
   */
  const filterFilesByFormat = (
    files: File[],
    format: string,
  ): FilteredFile[] => {
    return files.map(file => {
      let purpose = 'other'
      let required = false

      const fileName = file.name.toLowerCase()
      const fileExtension = fileName.split('.').pop() || ''

      // Determine file purpose based on name and extension
      if (fileName.includes('tokenizer')) {
        purpose = 'tokenizer'
        required = true
      } else if (fileName.endsWith('.json')) {
        if (fileName.includes('config')) {
          purpose = 'config'
          required = true
        } else {
          purpose = 'metadata'
        }
      } else if (fileName.endsWith('.txt')) {
        purpose = 'vocab'
      } else {
        // Check if it matches the selected format
        const formatOptions = LOCAL_FILE_TYPE_OPTIONS.find(
          opt => opt.value === format,
        )
        if (formatOptions?.extensions.includes(`.${fileExtension}`)) {
          purpose = 'model'
          required = true
        }
      }

      return { file, purpose, required }
    })
  }

  /**
   * Auto-detect file format from uploaded files
   */
  const detectFileFormat = (files: File[]): string => {
    // Check file extensions to detect format
    for (const file of files) {
      const fileName = file.name.toLowerCase()
      const extension = `.${fileName.split('.').pop()}`

      // Check each format type
      for (const formatOption of LOCAL_FILE_TYPE_OPTIONS) {
        if (formatOption.extensions.includes(extension)) {
          return formatOption.value
        }
      }
    }

    // Default to safetensors if no match found
    return 'safetensors'
  }

  /**
   * Handle folder upload
   */
  const handleFolderUpload = (info: any) => {
    const allFiles = info.fileList.map((item: any) => item.originFileObj)

    // Filter to only include files in the root folder (no subdirectories) and no dot files
    const rootFiles = allFiles.filter((file: File) => {
      // Check if file path contains '/' (subdirectory)
      const path = (file as any).webkitRelativePath || file.name
      const pathParts = path.split('/')

      // Get the filename (last part of the path)
      const filename = pathParts[pathParts.length - 1]

      // Filter out dot files (hidden files like .gitignore, .DS_Store)
      if (filename.startsWith('.')) {
        return false
      }

      // Only keep files that are in root (pathParts.length === 2: folderName/fileName)
      return pathParts.length === 2
    })

    setSelectedFiles(rootFiles)

    // Auto-detect file format from uploaded files and always update the form
    const detectedFormat = detectFileFormat(rootFiles)
    form.setFieldValue('file_format', detectedFormat)

    // Filter files based on detected format
    const filtered = filterFilesByFormat(rootFiles, detectedFormat)
    setFilteredFiles(filtered)

    // Auto-detect main file
    const mainFiles = filtered.filter(
      item => item.purpose === 'model' && item.required,
    )
    if (mainFiles.length > 0) {
      form.setFieldValue('main_filename', mainFiles[0].file.name)
    }

    // Update form field
    form.setFieldValue(
      'local_folder_path',
      `${rootFiles.length} files selected`,
    )
  }

  /**
   * Handle form submission
   */
  const handleSubmit = async () => {
    try {
      setLoading(true)
      clearUploadError()

      // Validate that files were selected (form validation doesn't catch this since we set a display string)
      if (selectedFiles.length === 0) {
        form.setFields([
          {
            name: 'local_folder_path',
            errors: ['Please select a model folder'],
          },
        ])
        setLoading(false)
        return
      }

      // Validate form fields (this will show inline errors for display_name, main_filename, etc.)
      // If validation fails, validateFields throws an error and Ant Design automatically shows the error messages
      let values
      try {
        values = await form.validateFields()
      } catch (_error) {
        // Form validation failed - errors are already displayed by Ant Design
        setLoading(false)
        return
      }

      // Auto-generate model ID from display name
      const modelId = generateModelId(values.display_name || 'model')

      // Comprehensive validation of selected files
      const validation = validateModelFiles(selectedFiles, values.file_format)

      if (!validation.isValid) {
        // Show file validation errors inline on the local_folder_path field
        form.setFields([
          {
            name: 'local_folder_path',
            errors: validation.errors,
          },
        ])
        setLoading(false)
        return
      }

      // Show warnings but allow upload to continue
      if (validation.warnings.length > 0) {
        validation.warnings.forEach(warning => {
          message.warning(warning)
        })
      }

      // Validate that the specified main file exists in filtered files
      const filesToUpload = filteredFiles.map(item => item.file)
      const mainFile = filesToUpload.find(
        file => file.name === values.main_filename,
      )
      if (!mainFile) {
        form.setFields([
          {
            name: 'main_filename',
            errors: ['Selected main file not found in uploaded files'],
          },
        ])
        setLoading(false)
        return
      }

      // Upload and auto-commit the files as a model in a single request
      await uploadLocalModel({
        name: modelId,
        provider_id: providerId!,
        display_name: values.display_name,
        description: values.description,
        main_filename: values.main_filename,
        file_format: values.file_format,
        capabilities: values.capabilities || {},
        engine_type: values.engine_type || 'mistralrs',
        engine_settings: values.engine_settings || {},
        files: filesToUpload,
      })

      message.success('Model uploaded successfully')

      // Reset and close
      form.resetFields()
      setSelectedFiles([])
      setFilteredFiles([])

      // Close drawer (imported from drawer-store)
      const { closeAddLocalLlmModelUploadDrawer } = await import(
        '../../store/llm-model-drawer-store'
      )
      closeAddLocalLlmModelUploadDrawer()

      // Note: Model will be added to provider automatically by the component's parent
      // when the drawer closes and the provider detail page refreshes
    } catch (error) {
      console.error('Failed to upload model:', error)
      message.error(
        error instanceof Error ? error.message : 'Failed to upload model',
      )
    } finally {
      setLoading(false)
    }
  }

  /**
   * Handle upload cancellation
   */
  const handleCancelUpload = () => {
    cancelUpload()
  }

  /**
   * Handle drawer close/cancel
   */
  const handleCancel = async () => {
    // Prevent closing if upload is in progress
    if (uploading) {
      message.warning(
        'Upload in progress - please cancel or wait for it to complete',
      )
      return
    }
    form.resetFields()
    setSelectedFiles([])
    setFilteredFiles([])

    const { closeAddLocalLlmModelUploadDrawer } = await import(
      '../../store/llm-model-drawer-store'
    )
    closeAddLocalLlmModelUploadDrawer()
  }

  // Update filtered files when format changes
  const file_format = Form.useWatch('file_format', form)
  useEffect(() => {
    if (selectedFiles.length > 0) {
      const newFilteredFiles = filterFilesByFormat(selectedFiles, file_format)
      setFilteredFiles(newFilteredFiles)

      // Update main file selection when format changes
      const mainFiles = newFilteredFiles.filter(
        item => item.purpose === 'model' && item.required,
      )
      if (mainFiles.length > 0) {
        // Auto-select first matching model file
        form.setFieldValue('main_filename', mainFiles[0].file.name)
      } else {
        // Clear selection if no matching model files
        form.setFieldValue('main_filename', undefined)
      }
    }
  }, [file_format, selectedFiles])

  return (
    <Drawer
      title="Upload Local Model"
      open={open}
      onClose={handleCancel}
      footer={[
        <Button key="cancel" onClick={handleCancel} disabled={uploading}>
          Cancel
        </Button>,
        <Button
          key="submit"
          type="primary"
          loading={loading}
          onClick={handleSubmit}
          disabled={uploading}
        >
          {uploading ? 'Uploading...' : 'Upload'}
        </Button>,
      ]}
      width={600}
      maskClosable={!uploading}
      closable={!uploading}
    >
      <Form
        name="llm-model-upload"
        form={form}
        layout="vertical"
        initialValues={{
          local_folder_path: '',
          main_filename: '',
          engine_type: 'mistralrs',
        }}
      >
        <LocalLlmModelCommonFields />

        <Form.Item
          name="local_folder_path"
          label="Model Folder"
          rules={[
            {
              required: true,
              message: 'Please select a model folder',
            },
          ]}
          valuePropName={'file'}
        >
          <Upload.Dragger
            directory
            multiple
            beforeUpload={() => false}
            onChange={handleFolderUpload}
            showUploadList={false}
            disabled={uploading}
          >
            <p className="ant-upload-drag-icon">
              <UploadOutlined />
            </p>
            <p className="ant-upload-text">
              Click or drag folder to select model files
            </p>
            <p className="ant-upload-hint">
              Only root folder files will be uploaded (subdirectories ignored)
            </p>
          </Upload.Dragger>
        </Form.Item>

        <Form.Item
          name="main_filename"
          label="Main Model File"
          rules={[
            {
              required: true,
              message: 'Please select the main model file',
            },
          ]}
        >
          <Select
            placeholder="Select the main model file"
            showSearch
            optionFilterProp="children"
            disabled={uploading}
            options={filteredFiles
              .filter(item => item.purpose === 'model')
              .map(item => ({
                value: item.file.name,
                label: item.file.name,
              }))}
          />
        </Form.Item>

        {!uploading && filteredFiles.length > 0 && (
          <Card title="Selected Files (Root Folder Only)" size="small">
            <List
              dataSource={filteredFiles}
              renderItem={item => (
                <List.Item
                  extra={
                    <div
                      style={{
                        display: 'flex',
                        gap: '8px',
                        alignItems: 'center',
                      }}
                    >
                      <Tag
                        color={item.required ? 'green' : 'blue'}
                        style={{ margin: 0 }}
                      >
                        {item.purpose}
                      </Tag>
                      <Text type="secondary" style={{ fontSize: '12px' }}>
                        {formatBytes(item.file.size)}
                      </Text>
                    </div>
                  }
                >
                  <Text
                    style={{
                      fontWeight: item.required ? 'bold' : 'normal',
                    }}
                  >
                    {item.file.name}
                  </Text>
                </List.Item>
              )}
            />
          </Card>
        )}

        {uploading &&
          (uploadProgress.length > 0 || overallUploadProgress > 0) && (
            <Card
              title="Upload Progress"
              size="small"
              extra={
                <Button
                  type="link"
                  danger
                  size="small"
                  onClick={handleCancelUpload}
                >
                  Cancel Upload
                </Button>
              }
            >
              {overallUploadProgress > 0 && (
                <div style={{ marginBottom: '12px' }}>
                  <Text strong>Overall Progress:</Text>
                  <Progress
                    percent={Math.round(overallUploadProgress)}
                    status="active"
                  />
                  <Text type="secondary" style={{ fontSize: '12px' }}>
                    {
                      uploadProgress.filter(f => f.status === 'completed')
                        .length
                    }{' '}
                    of {uploadProgress.length} files uploaded
                  </Text>
                </div>
              )}
              {uploadProgress.length > 0 && (
                <div>
                  {uploadProgress.map((fileProgress, index) => (
                    <div key={index} style={{ marginBottom: '8px' }}>
                      <Text strong>{fileProgress.filename}</Text>
                      <Progress
                        percent={Math.round(fileProgress.progress)}
                        status={
                          fileProgress.status === 'error'
                            ? 'exception'
                            : 'active'
                        }
                      />
                      {fileProgress.size && (
                        <Text type="secondary" style={{ fontSize: '12px' }}>
                          {formatBytes(
                            Math.round(
                              (fileProgress.progress * fileProgress.size) / 100,
                            ),
                          )}{' '}
                          of {formatBytes(fileProgress.size)} uploaded
                        </Text>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </Card>
          )}
      </Form>
    </Drawer>
  )
}
