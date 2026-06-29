/**
 * API helpers for LLM providers and models
 * These are used for fast test setup via direct API calls
 */

/**
 * Create an LLM provider via API
 * @param apiURL Base API URL
 * @param token Admin auth token
 * @param name Provider name
 * @param providerType Provider type (default: 'openai')
 * @returns Provider ID
 */
export async function createProviderViaAPI(
  apiURL: string,
  token: string,
  name: string,
  providerType: 'openai' | 'anthropic' | 'gemini' | 'groq' | 'local' = 'openai'
): Promise<string> {
  // Get API key from environment based on provider type
  const apiKeyMap = {
    openai: process.env.OPENAI_API_KEY,
    anthropic: process.env.ANTHROPIC_API_KEY,
    gemini: process.env.GEMINI_API_KEY,
    groq: process.env.GROQ_API_KEY,
    local: undefined,
  }

  const baseUrlMap = {
    openai: 'https://api.openai.com/v1',
    anthropic: 'https://api.anthropic.com/v1',
    gemini: 'https://generativelanguage.googleapis.com',
    groq: 'https://api.groq.com/openai/v1',
    local: 'http://localhost:11434',
  }

  // Local-bridge seam: when an override env var is set, point the provider at a
  // local OpenAI/Anthropic-compatible bridge (e.g. Qwen on http://localhost:4000/v1)
  // so real-LLM specs run without hitting the paid SaaS endpoint. Per-provider
  // override (ANTHROPIC_BASE_URL, OPENAI_BASE_URL, GEMINI_BASE_URL, GROQ_BASE_URL),
  // with a global fallback ZIEE_TEST_LLM_BASE_URL. If none set, the default
  // SaaS base_url above is used (unchanged behavior). The override must include
  // the path suffix the backend expects (Anthropic: .../v1, since the ziee
  // provider appends /messages).
  const baseUrlOverrideMap = {
    openai: process.env.OPENAI_BASE_URL,
    anthropic: process.env.ANTHROPIC_BASE_URL,
    gemini: process.env.GEMINI_BASE_URL,
    groq: process.env.GROQ_BASE_URL,
    local: process.env.LOCAL_BASE_URL,
  }
  const baseUrl =
    baseUrlOverrideMap[providerType] ||
    process.env.ZIEE_TEST_LLM_BASE_URL ||
    baseUrlMap[providerType]

  // The backend rejects an enabled REMOTE provider with an empty api_key
  // ("API key is required for enabled remote providers"). Structural specs
  // (routing, group assignment, …) only need a provider row to exist and don't
  // actually call the upstream, so fall back to a placeholder key when the
  // env key is unset. Real-LLM specs set the env/bridge key, which takes
  // precedence here. Local providers need no key.
  const apiKey =
    apiKeyMap[providerType] ??
    (providerType === 'local' ? undefined : 'sk-test-placeholder')

  const response = await fetch(`${apiURL}/api/llm-providers`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      name,
      provider_type: providerType,
      enabled: true,
      base_url: baseUrl,
      api_key: apiKey,
    }),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create provider: ${response.status} ${error}`)
  }

  const provider = await response.json()
  return provider.id
}

/**
 * Create an LLM model via API
 * @param apiURL Base API URL
 * @param token Admin auth token
 * @param providerId Provider ID
 * @param modelName Model name (e.g., 'gpt-4o-mini', 'claude-3-5-sonnet-20241022')
 * @param displayName Model display name (e.g., 'GPT-4o Mini')
 * @param providerType Provider type to determine default model if not specified
 * @returns Model ID
 */
export async function createModelViaAPI(
  apiURL: string,
  token: string,
  providerId: string,
  modelName?: string,
  displayName?: string,
  providerType: 'openai' | 'anthropic' | 'gemini' | 'groq' | 'local' = 'openai'
): Promise<string> {
  // Default models for each provider
  const defaultModels = {
    openai: { name: 'gpt-4o-mini', display: 'GPT-4o Mini' },
    anthropic: { name: 'claude-3-5-sonnet-20241022', display: 'Claude 3.5 Sonnet' },
    gemini: { name: 'gemini-1.5-flash', display: 'Gemini 1.5 Flash' },
    groq: { name: 'llama-3.1-8b-instant', display: 'Llama 3.1 8B' },
    local: { name: 'llama3', display: 'Llama 3' },
  }

  const defaultModel = defaultModels[providerType]
  const finalModelName = modelName || defaultModel.name
  const finalDisplayName = displayName || defaultModel.display

  const response = await fetch(`${apiURL}/api/llm-models`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      provider_id: providerId,
      name: finalModelName,
      display_name: finalDisplayName,
      enabled: true,
      engine_type: 'none',
      file_format: 'gguf',
      capabilities: {
        vision: false,
        function_calling: false,
        streaming: true,
      },
      parameters: {
        context_length: 4096,
        temperature: 0.7,
        top_p: 0.9,
        max_tokens: 2048,
      },
    }),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create model: ${response.status} ${error}`)
  }

  const model = await response.json()
  return model.id
}

/**
 * Get the Administrators group ID
 * @param apiURL Base API URL
 * @param token Admin auth token
 * @returns Administrators group ID
 */
export async function getAdministratorsGroupId(
  apiURL: string,
  token: string
): Promise<string> {
  const groupsResponse = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, {
    headers: {
      Authorization: `Bearer ${token}`,
    },
  })

  if (!groupsResponse.ok) {
    throw new Error(`Failed to fetch groups: ${groupsResponse.statusText}`)
  }

  const groupsData = await groupsResponse.json()
  const groups = Array.isArray(groupsData) ? groupsData : groupsData.groups || []

  const adminGroup = groups.find((g: any) => g.name === 'Administrators')

  if (!adminGroup) {
    throw new Error('Administrators group not found')
  }

  return adminGroup.id
}

/**
 * Assign a provider to the Administrators group
 * @param apiURL Base API URL
 * @param token Admin auth token
 * @param providerId Provider ID
 */
export async function assignProviderToAdministratorsGroup(
  apiURL: string,
  token: string,
  providerId: string
): Promise<void> {
  // First, get the Administrators group ID (with pagination params)
  const groupsResponse = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, {
    headers: {
      Authorization: `Bearer ${token}`,
    },
  })

  if (!groupsResponse.ok) {
    throw new Error(`Failed to fetch groups: ${groupsResponse.statusText}`)
  }

  const groupsData = await groupsResponse.json()
  console.log('[assignProviderToAdministratorsGroup] Groups API response:', JSON.stringify(groupsData, null, 2))

  const groups = Array.isArray(groupsData) ? groupsData : groupsData.groups || []
  console.log('[assignProviderToAdministratorsGroup] Parsed groups:', groups)

  const adminGroup = groups.find((g: any) => g.name === 'Administrators')
  console.log('[assignProviderToAdministratorsGroup] Admin group:', adminGroup)

  if (!adminGroup) {
    throw new Error('Administrators group not found')
  }

  // Assign the provider to the group
  const assignURL = `${apiURL}/api/groups/${adminGroup.id}/providers`
  console.log('[assignProviderToAdministratorsGroup] Assigning provider to:', assignURL)
  console.log('[assignProviderToAdministratorsGroup] Provider ID:', providerId)

  const response = await fetch(assignURL, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      provider_ids: [providerId],
    }),
  })

  console.log('[assignProviderToAdministratorsGroup] Response status:', response.status, response.statusText)

  if (!response.ok) {
    const error = await response.text()
    console.log('[assignProviderToAdministratorsGroup] Error response:', error)
    throw new Error(
      `Failed to assign provider to group: ${response.statusText} - ${error}`
    )
  }

  console.log('[assignProviderToAdministratorsGroup] Successfully assigned provider to group')
}

/**
 * Create a group via API
 * @param apiURL Base API URL
 * @param token Admin auth token
 * @param name Group name
 * @param description Group description
 * @returns Group ID
 */
export async function createGroupViaAPI(
  apiURL: string,
  token: string,
  name: string,
  description: string,
  permissions: string[] = []
): Promise<string> {
  const response = await fetch(`${apiURL}/api/groups`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      name,
      description,
      permissions,
    }),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create group: ${response.statusText} - ${error}`)
  }

  const group = await response.json()
  return group.id
}

/**
 * Assign a user to a group
 * @param apiURL Base API URL
 * @param token Admin auth token
 * @param userId User ID
 * @param groupId Group ID
 */
export async function assignUserToGroupViaAPI(
  apiURL: string,
  token: string,
  userId: string,
  groupId: string
): Promise<void> {
  const response = await fetch(`${apiURL}/api/groups/assign`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      user_id: userId,
      group_id: groupId,
    }),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(
      `Failed to assign user to group: ${response.statusText} - ${error}`
    )
  }
}

/**
 * Assign providers to a group
 * @param apiURL Base API URL
 * @param token Admin auth token
 * @param groupId Group ID
 * @param providerIds Array of provider IDs
 */
export async function assignProviderToGroupViaAPI(
  apiURL: string,
  token: string,
  groupId: string,
  providerIds: string[]
): Promise<void> {
  const response = await fetch(`${apiURL}/api/groups/${groupId}/providers`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      provider_ids: providerIds,
    }),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(
      `Failed to assign providers to group: ${response.statusText} - ${error}`
    )
  }
}

