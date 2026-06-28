import { Upload as UploadIcon } from 'lucide-react'
import {
  Button,
  Card,
  Form,
  FormField,
  List,
  Progress,
  Select,
  Tag,
  Text,
  Upload,
  message,
  useForm,
  useWatch,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { LOCAL_FILE_TYPE_OPTIONS } from '@/modules/llm-provider/constants'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { formatBytes } from '@/utils/downloadUtils'
import { LocalLlmModelCommonFields } from '@/modules/llm-provider/components/llm-models/shared/LocalLlmModelCommonFields'

/**
 * File with metadata for display
 */
interface FilteredFile {
  file: File
  purpose: string
  required: boolean
}

const schema = z
  .object({
    local_folder_path: z.string().min(1, 'Please select a model folder'),
    main_filename: z.string().min(1, 'Please select the main model file'),
  })
  .passthrough()

export function AddLocalLlmModelUploadDrawer() {
  const [loading, setLoading] = useState(false)
  const [selectedFiles, setSelectedFiles] = useState<File[]>([])
  const [filteredFiles, setFilteredFiles] = useState<FilteredFile[]>([])

  const { uploading, uploadProgress, overallUploadProgress } =
    Stores.LlmModelUpload
  const { open, providerId } = Stores.AddLocalLlmModelUploadDrawer
  const canCreate = usePermission(Permissions.LlmModelsCreate)

  const form = useForm<any>({
    resolver: zodResolver(schema),
    defaultValues: {
      local_folder_path: '',
      main_filename: '',
      engine_type: 'mistralrs',
    },
  })

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
   * Handle folder / file selection (raw File[] from the kit Upload dropzone).
   */
  const handleFiles = (allFiles: File[]) => {
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

      // Keep root-level files: a plain file selection (pathParts.length === 1)
      // or a folder's root file (folderName/fileName === 2). Deeper paths are
      // subdirectory files and are ignored.
      return pathParts.length <= 2
    })

    setSelectedFiles(rootFiles)

    // Auto-detect file format from uploaded files and always update the form
    const detectedFormat = detectFileFormat(rootFiles)
    form.setValue('file_format', detectedFormat)

    // Filter files based on detected format
    const filtered = filterFilesByFormat(rootFiles, detectedFormat)
    setFilteredFiles(filtered)

    // Auto-detect main file
    const mainFiles = filtered.filter(
      item => item.purpose === 'model' && item.required,
    )
    if (mainFiles.length > 0) {
      form.setValue('main_filename', mainFiles[0].file.name)
    }

    // Update form field
    form.setValue('local_folder_path', `${rootFiles.length} files selected`)
  }

  /**
   * Handle form submission
   */
  const onSubmit = async (values: any) => {
    try {
      setLoading(true)
      Stores.LlmModelUpload.clearUploadError()

      // Validate that files were selected (schema only sees the display string)
      if (selectedFiles.length === 0) {
        form.setError('local_folder_path', {
          type: 'manual',
          message: 'Please select a model folder',
        })
        setLoading(false)
        return
      }

      // Auto-generate model ID from display name
      const modelId = generateModelId(values.display_name || 'model')

      // Comprehensive validation of selected files
      const validation = validateModelFiles(selectedFiles, values.file_format)

      if (!validation.isValid) {
        // Show file validation errors inline on the local_folder_path field
        form.setError('local_folder_path', {
          type: 'manual',
          message: validation.errors.join(' '),
        })
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
        form.setError('main_filename', {
          type: 'manual',
          message: 'Selected main file not found in uploaded files',
        })
        setLoading(false)
        return
      }

      // Upload and auto-commit the files as a model in a single request
      await Stores.LlmModelUpload.uploadLocalModel({
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
      form.reset()
      setSelectedFiles([])
      setFilteredFiles([])

      // Close drawer
      Stores.AddLocalLlmModelUploadDrawer.closeAddLocalLlmModelUploadDrawer()

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
    Stores.LlmModelUpload.cancelUpload()
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
    form.reset()
    setSelectedFiles([])
    setFilteredFiles([])

    Stores.AddLocalLlmModelUploadDrawer.closeAddLocalLlmModelUploadDrawer()
  }

  // Update filtered files when format changes
  const file_format = useWatch({ control: form.control, name: 'file_format' })
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
        form.setValue('main_filename', mainFiles[0].file.name)
      } else {
        // Clear selection if no matching model files
        form.setValue('main_filename', undefined)
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file_format, selectedFiles])

  return (
    <Drawer
      title="Upload Local Model"
      open={open}
      onClose={handleCancel}
      footer={[
        <Button
          key="cancel"
          variant="outline"
          onClick={handleCancel}
          disabled={uploading}
        >
          {canCreate ? 'Cancel' : 'Close'}
        </Button>,
        canCreate && (
          <Button
            key="submit"
            loading={loading}
            onClick={form.handleSubmit(onSubmit)}
            disabled={uploading}
          >
            {uploading ? 'Uploading...' : 'Upload'}
          </Button>
        ),
      ]}
      size={600}
      mask={{ closable: !uploading }}
      closable={!uploading}
    >
      <Form
        name="llm-model-upload"
        form={form}
        onSubmit={onSubmit}
        layout="vertical"
      >
        <LocalLlmModelCommonFields />

        <FormField name="local_folder_path" label="Model Folder" required>
          <Upload
            label="Click or drag folder to select model files"
            multiple
            directory
            disabled={uploading}
            onFiles={handleFiles}
          >
            <UploadIcon />
            <p>Click or drag folder to select model files</p>
            <p className="text-muted-foreground text-xs">
              Only root folder files will be uploaded (subdirectories ignored)
            </p>
          </Upload>
        </FormField>

        <FormField name="main_filename" label="Main Model File" required>
          <Select
            placeholder="Select the main model file"
            disabled={uploading}
            options={filteredFiles
              .filter(item => item.purpose === 'model')
              .map(item => ({
                value: item.file.name,
                label: item.file.name,
              }))}
          />
        </FormField>

        {!uploading && filteredFiles.length > 0 && (
          <Card title="Selected Files (Root Folder Only)" size="sm">
            <List
              dataSource={filteredFiles}
              rowKey={item => item.file.name}
              renderItem={item => (
                <div className="flex items-center justify-between">
                  <Text className={item.required ? 'font-bold' : undefined}>
                    {item.file.name}
                  </Text>
                  <div className="flex gap-2 items-center">
                    <Tag tone={item.required ? 'success' : 'info'}>
                      {item.purpose}
                    </Tag>
                    <Text type="secondary" className="text-xs">
                      {formatBytes(item.file.size)}
                    </Text>
                  </div>
                </div>
              )}
            />
          </Card>
        )}

        {uploading &&
          (uploadProgress.length > 0 || overallUploadProgress > 0) && (
            <Card
              title="Upload Progress"
              size="sm"
              extra={
                <Button
                  variant="link"
                  size="sm"
                  className="text-destructive"
                  onClick={handleCancelUpload}
                >
                  Cancel Upload
                </Button>
              }
            >
              {overallUploadProgress > 0 && (
                <div className="mb-3">
                  <Text strong>Overall Progress:</Text>
                  <Progress
                    aria-label="Overall upload progress"
                    value={Math.round(overallUploadProgress)}
                    tone="primary"
                  />
                  <Text type="secondary" className="text-xs">
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
                    <div key={index} className="mb-2">
                      <Text strong>{fileProgress.filename}</Text>
                      <Progress
                        aria-label={`Upload progress for ${fileProgress.filename}`}
                        value={Math.round(fileProgress.progress)}
                        tone={
                          fileProgress.status === 'error' ? 'error' : 'primary'
                        }
                      />
                      {fileProgress.size && (
                        <Text type="secondary" className="text-xs">
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
