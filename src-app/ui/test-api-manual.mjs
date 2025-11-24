// Manual API test to verify provider assignment chain
const apiURL = 'http://localhost:54123'

async function testProviderAssignment() {
  console.log('1. Logging in as admin...')
  const loginResponse = await fetch(`${apiURL}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      username: 'admin',
      password: 'password123',
    }),
  })
  const loginData = await loginResponse.json()
  const token = loginData.access_token
  console.log('✓ Login successful, got token')

  console.log('\n2. Creating provider...')
  const providerResponse = await fetch(`${apiURL}/api/llm-providers`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      name: 'Test Provider',
      provider_type: 'local',
      enabled: true,
    }),
  })
  const providerData = await providerResponse.json()
  const providerId = providerData.id
  console.log(`✓ Provider created: ${providerId}`)

  console.log('\n3. Getting Administrators group...')
  const groupsResponse = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  const groupsData = await groupsResponse.json()
  const adminGroup = groupsData.groups.find(g => g.name === 'Administrators')
  console.log(`✓ Administrators group found: ${adminGroup.id}`)

  console.log('\n4. Assigning provider to Administrators group...')
  const assignResponse = await fetch(`${apiURL}/api/groups/${adminGroup.id}/providers`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      provider_ids: [providerId],
    }),
  })
  if (!assignResponse.ok) {
    const text = await assignResponse.text()
    console.error(`✗ Assignment failed: ${assignResponse.status} ${assignResponse.statusText} - ${text}`)
    return
  }
  console.log('✓ Provider assigned to group')

  console.log('\n5. Creating model...')
  const modelResponse = await fetch(`${apiURL}/api/llm-models`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      provider_id: providerId,
      name: 'test-model',
      display_name: 'Test Model',
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
  const modelData = await modelResponse.json()
  console.log(`✓ Model created: ${modelData.id}`)

  console.log('\n6. Fetching user-accessible providers...')
  const userProvidersResponse = await fetch(`${apiURL}/api/chat/llm-providers`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  const userProvidersData = await userProvidersResponse.json()
  console.log(`✓ Got ${userProvidersData.providers.length} providers`)

  if (userProvidersData.providers.length === 0) {
    console.error('✗ NO PROVIDERS RETURNED - THIS IS THE BUG!')
  } else {
    console.log('✓ Providers returned:')
    for (const provider of userProvidersData.providers) {
      console.log(`  - ${provider.provider.name} (${provider.llm_models.length} models)`)
      for (const model of provider.llm_models) {
        console.log(`    - ${model.display_name}`)
      }
    }
  }
}

testProviderAssignment().catch(console.error)
