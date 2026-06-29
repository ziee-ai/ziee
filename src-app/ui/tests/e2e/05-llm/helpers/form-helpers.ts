import { Page, expect, Locator } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * LLM-specific form helpers (kit / data-testid based)
 */

// =====================================================
// Kit primitive helpers
// =====================================================

/** Open a kit Select (by trigger testid) and pick the option whose value is `value`. */
async function selectKitOptionByValue(page: Page, selectTestId: string, value: string) {
  await byTestId(page, selectTestId).click()
  await page.getByTestId(`${selectTestId}-opt-${value}`).click()
}

/** Open a kit Select and pick the option matching the (dynamic) visible label text. */
async function selectKitOptionByLabel(page: Page, selectTestId: string, label: string) {
  await byTestId(page, selectTestId).click()
  await page
    .locator(`[data-testid^="${selectTestId}-opt-"]`)
    .filter({ hasText: label })
    .first()
    .click()
}

/** Set a kit Switch (role="switch") to the desired checked state. */
async function setKitSwitch(locator: Locator, desired: boolean) {
  const checked = (await locator.getAttribute('aria-checked')) === 'true'
  if (checked !== desired) {
    await locator.click()
  }
}

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
  if (data.name !== undefined) {
    await byTestId(page, 'llm-provider-name-input').fill(data.name)
  }

  // Enabled switch (optional)
  if (data.enabled !== undefined) {
    await setKitSwitch(byTestId(page, 'llm-provider-enabled-switch'), data.enabled)
  }

  // Remote provider fields
  if (data.baseUrl) {
    await byTestId(page, 'llm-provider-base-url-input').fill(data.baseUrl)
  }

  if (data.apiKey) {
    await byTestId(page, 'llm-provider-api-key-input').fill(data.apiKey)
  }
}

export async function submitProviderForm(page: Page) {
  await byTestId(page, 'llm-provider-submit-btn').click()
  await page.waitForLoadState('load')
}

export async function updateProviderForm(page: Page) {
  await byTestId(page, 'llm-provider-submit-btn').click()
  await page.waitForLoadState('load')
}

export async function cancelProviderForm(page: Page) {
  await byTestId(page, 'llm-provider-cancel-btn').click()
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

export async function fillModelCommonFields(page: Page, data: ModelFormData, _formName: string = '') {
  void _formName
  await byTestId(page, 'llm-param-display_name').fill(data.displayName)

  if (data.description) {
    await byTestId(page, 'llm-param-description').fill(data.description)
  }

  // Engine type + file format are kit Selects.
  await selectKitOptionByValue(page, 'llm-engine-type-select', data.engineType)
  await selectKitOptionByValue(page, 'llm-file-format-select', data.fileFormat)
}

export async function fillModelCapabilities(page: Page, data: ModelFormData) {
  // Capability switches: `llm-capability-switch-${name}`. Component uses
  // `codeInterpreter` / `image_generator` / `text_embedding` etc.
  const capabilities: Array<[string, boolean | undefined]> = [
    ['chat', data.chat],
    ['text_embedding', data.textEmbedding],
    ['codeInterpreter', data.codeInterpreter],
    ['vision', data.vision],
    ['audio', data.audio],
    ['image_generator', data.imageGenerator],
    ['tools', data.tools],
  ]

  for (const [name, value] of capabilities) {
    if (value !== undefined) {
      const sw = byTestId(page, `llm-capability-switch-${name}`)
      if (await sw.count()) {
        await setKitSwitch(sw.first(), value)
      }
    }
  }
}

export async function fillModelParameters(page: Page, data: ModelFormData) {
  if (data.maxTokens) {
    await byTestId(page, 'llm-param-parameters.max_tokens').fill(data.maxTokens.toString())
  }
  if (data.temperature) {
    await byTestId(page, 'llm-param-parameters.temperature').fill(data.temperature.toString())
  }
  if (data.topP) {
    await byTestId(page, 'llm-param-parameters.top_p').fill(data.topP.toString())
  }
  if (data.topK) {
    await byTestId(page, 'llm-param-parameters.top_k').fill(data.topK.toString())
  }
}

export async function fillModelEngineSettings(page: Page, data: ModelFormData) {
  const deviceSelect =
    data.engineType === 'mistralrs' ? 'llm-mistralrs-device-type' : 'llm-llamacpp-device-type'

  if (data.deviceType) {
    const sel = byTestId(page, deviceSelect)
    if (await sel.count()) {
      await selectKitOptionByValue(page, deviceSelect, data.deviceType)
    }
  }

  // NOTE: MistralRS "command" select has no dedicated testid in the kit
  // form; left as a no-op for signature compatibility.
  void data.command

  // LlamaCPP specific
  if (data.engineType === 'llamacpp') {
    if (data.contextSize) {
      const f = byTestId(page, 'llm-llamacpp-ctx-size')
      if (await f.count()) await f.fill(data.contextSize.toString())
    }
    if (data.gpuLayers !== undefined) {
      const f = byTestId(page, 'llm-llamacpp-n-gpu-layers')
      if (await f.count()) await f.fill(data.gpuLayers.toString())
    }
    if (data.useMmap !== undefined) {
      const sw = byTestId(page, 'llm-llamacpp-no-mmap')
      // `no-mmap` is the inverse of useMmap.
      if (await sw.count()) await setKitSwitch(sw.first(), !data.useMmap)
    }
    if (data.useMlock !== undefined) {
      const sw = byTestId(page, 'llm-llamacpp-mlock')
      if (await sw.count()) await setKitSwitch(sw.first(), data.useMlock)
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

  // Repository selection — option values are repository UUIDs; their
  // labels carry the repository name.
  const isUUID = data.repositoryId.includes('-') && data.repositoryId.length > 20
  if (isUUID) {
    await selectKitOptionByValue(page, 'llm-download-repository-select', data.repositoryId)
  } else {
    const nameMap: Record<string, string> = {
      huggingface: 'Hugging Face Hub',
      github: 'GitHub',
    }
    const repoName = nameMap[data.repositoryId.toLowerCase()] || data.repositoryId
    await selectKitOptionByLabel(page, 'llm-download-repository-select', repoName)
  }

  await byTestId(page, 'llm-download-repository-path-input').fill(data.repositoryPath)
  await byTestId(page, 'llm-download-main-filename-input').fill(data.mainFilename)

  if (data.branch) {
    await byTestId(page, 'llm-download-branch-input').fill(data.branch)
  }

  // Clear-cache field was removed in security remediation (07-llm-model
  // F-17). Tests passing `clearCache` now no-op.
  void data.clearCache

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
// Repository Form Helpers (legacy shape; see repository-helpers.ts for
// the canonical one used by repo specs)
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
  await byTestId(page, 'llmrepo-form-name').fill(data.name)
  await byTestId(page, 'llmrepo-form-url').fill(data.url)

  const authValue =
    data.authType === 'token' ? 'bearer_token' : data.authType === 'basic' ? 'basic_auth' : 'none'
  await selectKitOptionByValue(page, 'llmrepo-form-auth-type', authValue)

  if (data.authType === 'token' && data.token) {
    await byTestId(page, 'llmrepo-form-token').fill(data.token)
  }

  if (data.authType === 'basic') {
    if (data.username) {
      await byTestId(page, 'llmrepo-form-username').fill(data.username)
    }
    if (data.password) {
      await byTestId(page, 'llmrepo-form-password').fill(data.password)
    }
  }

  if (data.enabled !== undefined) {
    await setKitSwitch(byTestId(page, 'llmrepo-form-enabled-switch'), data.enabled)
  }
}

export async function submitRepositoryForm(page: Page) {
  await byTestId(page, 'llmrepo-form-submit-btn').click()
  await page.waitForLoadState('load')
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

  // After files are uploaded, select main filename from the kit Select.
  if (data.mainFilename) {
    await byTestId(page, 'llm-upload-main-file-select').waitFor({ timeout: 5000 })
    await selectKitOptionByLabel(page, 'llm-upload-main-file-select', data.mainFilename)
  }

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
  const uploadButton = byTestId(page, 'llm-upload-drawer-submit-btn')
  await expect(uploadButton).toBeEnabled()
  await uploadButton.click()
  // After clicking, the button should enter loading state quickly.
  await page.waitForTimeout(500)
}
