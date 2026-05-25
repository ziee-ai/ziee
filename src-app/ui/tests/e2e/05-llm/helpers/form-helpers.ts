import { Page, expect } from '@playwright/test'

/**
 * LLM-specific form helpers
 */

// =====================================================
// Provider Form Helpers
// =====================================================

export interface ProviderFormData {
  name: string
  description?: string
  enabled?: boolean
  // Remote provider specific
  baseUrl?: string
  apiKey?: string
  // Proxy settings
  proxyEnabled?: boolean
  proxyUrl?: string
  proxyUsername?: string
  proxyPassword?: string
}

export async function fillProviderForm(page: Page, data: ProviderFormData) {
  // With Form name="llm-provider-form", field IDs are prefixed with the form name
  // e.g., llm-provider-form_name instead of just name
  await page.fill('#llm-provider-form_name', data.name)

  // NOTE: Add Provider drawer does NOT have a description field
  // Description is only used in other contexts

  // Enabled checkbox (optional)
  if (data.enabled !== undefined) {
    const checkbox = page.locator('#llm-provider-form_enabled')
    const isChecked = await checkbox.isChecked()
    if (isChecked !== data.enabled) {
      await checkbox.click()
    }
  }

  // Remote provider fields
  if (data.baseUrl) {
    await page.fill('#llm-provider-form_base_url', data.baseUrl)
  }

  if (data.apiKey) {
    await page.fill('#llm-provider-form_api_key', data.apiKey)
  }

  // Proxy settings (these would be on a different form - provider-proxy-form)
  if (data.proxyEnabled !== undefined) {
    const proxyCheckbox = page.locator('#provider-proxy-form_enabled')
    const isChecked = await proxyCheckbox.isChecked()
    if (isChecked !== data.proxyEnabled) {
      await proxyCheckbox.click()
    }

    if (data.proxyEnabled) {
      if (data.proxyUrl) {
        await page.fill('#provider-proxy-form_url', data.proxyUrl)
      }
      if (data.proxyUsername) {
        await page.fill('#provider-proxy-form_username', data.proxyUsername)
      }
      if (data.proxyPassword) {
        await page.fill('#provider-proxy-form_password', data.proxyPassword)
      }
    }
  }
}

export async function submitProviderForm(page: Page) {
  // Wait for any dropdown overlay to dismiss. AntD's Select dropdowns
  // can leave invisible overlays that block normal click; submit
  // via Form's keyboard handler instead — press Enter on the
  // submit button after focusing.
  await page.waitForTimeout(500)
  const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
  const submitButton = drawer.locator('button[type="submit"]:has-text("Add Provider")')
  await submitButton.focus()
  await submitButton.press('Enter')
  await page.waitForLoadState('networkidle')
}

export async function updateProviderForm(page: Page) {
  await page.waitForTimeout(500)
  const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
  const submitButton = drawer.locator('button[type="submit"]:has-text("Update Provider")')
  await submitButton.focus()
  await submitButton.press('Enter')
  await page.waitForLoadState('networkidle')
}

export async function cancelProviderForm(page: Page) {
  await page.click('button:has-text("Cancel")')
}

// =====================================================
// Model Form Helpers
// =====================================================

export interface ModelFormData {
  displayName: string
  description?: string
  fileFormat: 'safetensors' | 'gguf' | 'pytorch' | 'ggml'
  engineType: 'mistralrs' | 'llamacpp'
  // Capabilities
  chat?: boolean
  textEmbedding?: boolean
  codeInterpreter?: boolean
  vision?: boolean
  audio?: boolean
  imageGenerator?: boolean
  tools?: boolean
  // Parameters
  maxTokens?: number
  temperature?: number
  topP?: number
  topK?: number
  // Engine settings
  deviceType?: 'cpu' | 'cuda' | 'metal' | 'rocm' | 'vulkan' | 'opencl' | 'auto'
  // MistralRS specific
  command?: 'plain' | 'gguf' | 'run' | 'vision-plain' | 'x-lora' | 'lora' | 'toml'
  // LlamaCPP specific
  contextSize?: number
  gpuLayers?: number
  useMmap?: boolean
  useMlock?: boolean
}

export async function fillModelCommonFields(page: Page, data: ModelFormData, formName: string = '') {
  const prefix = formName ? `${formName}_` : ''

  await page.fill(`#${prefix}display_name`, data.displayName)

  if (data.description) {
    await page.fill(`#${prefix}description`, data.description)
  }

  // File format dropdown - Ant Design Select requires clicking on the .ant-select wrapper
  // We use the input ID to find the correct .ant-select
  const fileFormatSelect = page.locator(`.ant-select:has(input#${prefix}file_format)`)
  await fileFormatSelect.click()
  await page.click(`text=${data.fileFormat}`)

  // Engine type dropdown - Ant Design Select requires clicking on the .ant-select wrapper
  const engineTypeSelect = page.locator(`.ant-select:has(input#${prefix}engine_type)`)
  await engineTypeSelect.click()
  await page.click(`text=${data.engineType}`)
}

export async function fillModelCapabilities(page: Page, data: ModelFormData) {
  // Expand capabilities section if collapsed
  const capabilitiesSection = page.locator('text=Capabilities').first()
  await capabilitiesSection.click()

  const capabilities = {
    chat: data.chat,
    text_embedding: data.textEmbedding,
    code_interpreter: data.codeInterpreter,
    vision: data.vision,
    audio: data.audio,
    image_generator: data.imageGenerator,
    tools: data.tools,
  }

  for (const [key, value] of Object.entries(capabilities)) {
    if (value !== undefined) {
      const checkbox = page.locator(`#capabilities_${key}`)
      const isChecked = await checkbox.isChecked()
      if (isChecked !== value) {
        await checkbox.click()
      }
    }
  }
}

export async function fillModelParameters(page: Page, data: ModelFormData) {
  // Expand parameters section if collapsed
  const parametersSection = page.locator('text=Parameters').first()
  await parametersSection.click()

  if (data.maxTokens) {
    await page.fill('#parameters_max_tokens', data.maxTokens.toString())
  }

  if (data.temperature) {
    await page.fill('#parameters_temperature', data.temperature.toString())
  }

  if (data.topP) {
    await page.fill('#parameters_top_p', data.topP.toString())
  }

  if (data.topK) {
    await page.fill('#parameters_top_k', data.topK.toString())
  }
}

export async function fillModelEngineSettings(page: Page, data: ModelFormData) {
  if (data.deviceType) {
    await page.click('.ant-select:has-text("Device Type")')
    await page.click(`text=${data.deviceType}`)
  }

  // MistralRS specific
  if (data.engineType === 'mistralrs' && data.command) {
    await page.click('.ant-select:has-text("Command")')
    await page.click(`text=${data.command}`)
  }

  // LlamaCPP specific
  if (data.engineType === 'llamacpp') {
    if (data.contextSize) {
      await page.fill('#engine_settings_context_size', data.contextSize.toString())
    }

    if (data.gpuLayers !== undefined) {
      await page.fill('#engine_settings_gpu_layers', data.gpuLayers.toString())
    }

    if (data.useMmap !== undefined) {
      const checkbox = page.locator('#engine_settings_use_mmap')
      const isChecked = await checkbox.isChecked()
      if (isChecked !== data.useMmap) {
        await checkbox.click()
      }
    }

    if (data.useMlock !== undefined) {
      const checkbox = page.locator('#engine_settings_use_mlock')
      const isChecked = await checkbox.isChecked()
      if (isChecked !== data.useMlock) {
        await checkbox.click()
      }
    }
  }
}

// =====================================================
// Download Form Helpers
// =====================================================

export interface DownloadFormData extends ModelFormData {
  repositoryId: string
  repositoryPath: string
  mainFilename: string
  branch?: string
  clearCache?: boolean
}

export async function fillDownloadForm(page: Page, data: DownloadFormData) {
  // Fill common model fields
  await fillModelCommonFields(page, data, 'llm-model-download')

  // Repository selection - Ant Design Select requires clicking on the .ant-select wrapper
  const repositorySelect = page.locator('.ant-select:has(input#llm-model-download_repository_id)')
  await repositorySelect.click()
  // Wait for dropdown to appear
  await page.waitForSelector(`.ant-select-dropdown:not(.ant-select-dropdown-hidden)`)

  // Click the option - support both UUID and name-based selection
  // For tests using repository names like "huggingface", match partial text in the title
  // The title format is: "Repository Name (URL)"
  const isUUID = data.repositoryId.includes('-') && data.repositoryId.length > 20
  if (isUUID) {
    // If it's a UUID, we can't match by text, so select the first option for now
    await page.locator('.ant-select-item:not(.ant-select-item-option-disabled)').first().click()
  } else {
    // For name-based selection (like "huggingface"), match the repository by name
    // Map common test identifiers to repository names
    const nameMap: Record<string, string> = {
      'huggingface': 'Hugging Face Hub',
      'github': 'GitHub'
    }
    const repoName = nameMap[data.repositoryId.toLowerCase()] || data.repositoryId
    await page.click(`.ant-select-item:has-text("${repoName}")`)
  }

  // Repository path - use prefixed ID
  await page.fill('#llm-model-download_repository_path', data.repositoryPath)

  // Main filename - use prefixed ID
  await page.fill('#llm-model-download_main_filename', data.mainFilename)

  // Branch (optional) - use prefixed ID
  if (data.branch) {
    await page.fill('#llm-model-download_repository_branch', data.branch)
  }

  // Clear-cache field was removed in security remediation (07-llm-model
  // F-17): the flag allowed any download-permitted user to wipe cache
  // for any model. Tests passing `clearCache` now no-op.
  void data.clearCache

  // Fill capabilities, parameters, engine settings
  if (data.chat !== undefined || data.textEmbedding !== undefined) {
    await fillModelCapabilities(page, data)
  }

  if (data.maxTokens || data.temperature) {
    await fillModelParameters(page, data)
  }

  if (data.deviceType || data.command) {
    await fillModelEngineSettings(page, data)
  }
}

// =====================================================
// Repository Form Helpers
// =====================================================

export interface RepositoryFormData {
  name: string
  url: string
  authType: 'none' | 'token' | 'basic'
  enabled?: boolean
  // Auth fields
  token?: string
  username?: string
  password?: string
}

export async function fillRepositoryForm(page: Page, data: RepositoryFormData) {
  await page.fill('#name', data.name)
  await page.fill('#url', data.url)

  // Auth type dropdown
  await page.click('.ant-select:has-text("Auth Type")')
  await page.click(`text=${data.authType}`)

  // Auth fields based on type
  if (data.authType === 'token' && data.token) {
    await page.fill('#token', data.token)
  }

  if (data.authType === 'basic') {
    if (data.username) {
      await page.fill('#username', data.username)
    }
    if (data.password) {
      await page.fill('#password', data.password)
    }
  }

  // Enabled checkbox
  if (data.enabled !== undefined) {
    const checkbox = page.locator('#enabled')
    const isChecked = await checkbox.isChecked()
    if (isChecked !== data.enabled) {
      await checkbox.click()
    }
  }
}

export async function submitRepositoryForm(page: Page) {
  await page.click('button[type="submit"]')
  await page.waitForLoadState('networkidle')
}

// =====================================================
// Upload Form Helpers
// =====================================================

export interface UploadFormData extends ModelFormData {
  folderPath: string
  mainFilename: string
}

export async function fillUploadForm(page: Page, data: UploadFormData) {
  // Fill common model fields
  await fillModelCommonFields(page, data, 'llm-model-upload')

  // File format should already be selected via fillModelCommonFields
  // Now we need to upload the folder

  // Note: The actual folder upload happens via file input interaction
  // This will be handled by the test directly using page.setInputFiles()

  // After files are uploaded, select main filename from dropdown
  if (data.mainFilename) {
    await page.waitForSelector('.ant-select:has-text("Main Model File")', { timeout: 5000 })
    await page.click('.ant-select:has-text("Main Model File")')
    await page.click(`text=${data.mainFilename}`)
  }

  // Fill capabilities, parameters, engine settings if provided
  if (data.chat !== undefined || data.textEmbedding !== undefined) {
    await fillModelCapabilities(page, data)
  }

  if (data.maxTokens || data.temperature) {
    await fillModelParameters(page, data)
  }

  if (data.deviceType || data.command) {
    await fillModelEngineSettings(page, data)
  }
}

export async function submitUploadForm(page: Page) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
  // Use exact-name match — the dropzone also exposes a button-role
  // element with "Upload" in its accessible name; `:has-text("Upload")`
  // matches both the dropzone span and the submit button.
  const uploadButton = drawer.getByRole('button', { name: 'Upload', exact: true })

  // Ensure button is enabled before clicking
  await expect(uploadButton).toBeEnabled()

  await uploadButton.click()

  // After clicking, the button should enter loading state quickly
  // Wait a bit to ensure the upload has started
  await page.waitForTimeout(500)
}
