import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, login, createTestUser, clearAuthState } from '../../common/auth-helpers'
import {
  assignProviderToAdministratorsGroup,
  getAdministratorsGroupId,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  getVisibleModelsInDropdown,
  assertModelVisibleInDropdown,
  assertModelNotVisibleInDropdown,
  assertDropdownEmpty,
} from './helpers/chat-helpers'

/**
 * Chat Access Control E2E Tests
 *
 * Tests that verify users can only see and use models they have access to via group assignments
 */

test.describe('Chat - Model Access Control', () => {
  test('admin user sees all enabled models in dropdown', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Ensure admin exists
    await loginAsAdmin(page, baseURL)

    // Create providers and models via API
    const adminToken = await getAdminToken(apiURL)

    // Create provider 1 with 2 models
    const provider1Id = await createProviderViaAPI(apiURL, adminToken, 'Test Provider 1')
    await createModelViaAPI(apiURL, adminToken, provider1Id, 'Model A', 'Test Model A')
    await createModelViaAPI(apiURL, adminToken, provider1Id, 'Model B', 'Test Model B')

    // Create provider 2 with 1 model
    const provider2Id = await createProviderViaAPI(apiURL, adminToken, 'Test Provider 2')
    await createModelViaAPI(apiURL, adminToken, provider2Id, 'Model C', 'Test Model C')

    // Assign BOTH providers to the Administrators group in a single call.
    // PUT /api/groups/{id}/providers replaces the whole list — calling
    // assignProviderToAdministratorsGroup twice would wipe the first assignment.
    const adminGroupId = await getAdministratorsGroupId(apiURL, adminToken)
    await assignProviderToGroupViaAPI(apiURL, adminToken, adminGroupId, [provider1Id, provider2Id])

    // Logout and login again so store loads with newly created models
    await clearAuthState(page)
    await loginAsAdmin(page, baseURL)
    await goToNewChatPage(page, baseURL)

    // Verify all models are visible
    const visibleModels = await getVisibleModelsInDropdown(page)
    expect(visibleModels).toContain('Test Model A')
    expect(visibleModels).toContain('Test Model B')
    expect(visibleModels).toContain('Test Model C')
  })

  test('user only sees models from assigned groups', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Ensure admin exists
    await loginAsAdmin(page, baseURL)

    // Setup: Create providers, models, and groups
    const adminToken = await getAdminToken(apiURL)

    // Create provider 1 with model A (will be assigned to group)
    const provider1Id = await createProviderViaAPI(apiURL, adminToken, 'Provider A')
    await createModelViaAPI(apiURL, adminToken, provider1Id, 'model-a', 'Model A')

    // Create provider 2 with model B (will NOT be assigned to group)
    const provider2Id = await createProviderViaAPI(apiURL, adminToken, 'Provider B')
    await createModelViaAPI(apiURL, adminToken, provider2Id, 'model-b', 'Model B')

    // Create test user
    const username = `testuser_${Date.now()}`
    const password = 'TestPass123!'
    const userId = await createTestUser(apiURL, adminToken, username, `${username}@test.com`, password)

    // Create group and assign provider 1 to it
    const groupId = await createGroupViaAPI(apiURL, adminToken, `test-group-${Date.now()}`, 'Test group')
    await assignProviderToGroupViaAPI(apiURL, adminToken, groupId, [provider1Id])

    // Assign user to group
    await assignUserToGroupViaAPI(apiURL, adminToken, userId, groupId)

    // Login as test user after creating models/groups so store loads with correct data
    await login(page, baseURL, username, password)
    await goToNewChatPage(page, baseURL)

    // Verify user only sees Model A (from assigned provider)
    await assertModelVisibleInDropdown(page, 'Model A')
    await assertModelNotVisibleInDropdown(page, 'Model B')
  })

  test('user in multiple groups sees models from all groups', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Ensure admin exists
    await loginAsAdmin(page, baseURL)

    // Setup
    const adminToken = await getAdminToken(apiURL)

    // Create provider 1 with model A
    const provider1Id = await createProviderViaAPI(apiURL, adminToken, 'Provider X')
    await createModelViaAPI(apiURL, adminToken, provider1Id, 'model-x', 'Model X')

    // Create provider 2 with model B
    const provider2Id = await createProviderViaAPI(apiURL, adminToken, 'Provider Y')
    await createModelViaAPI(apiURL, adminToken, provider2Id, 'model-y', 'Model Y')

    // Create test user
    const username = `multiuser_${Date.now()}`
    const password = 'TestPass123!'
    const userId = await createTestUser(apiURL, adminToken, username, `${username}@test.com`, password)

    // Create two groups via API
    const group1Id = await createGroupViaAPI(apiURL, adminToken, `group-1-${Date.now()}`, 'Test group 1')
    const group2Id = await createGroupViaAPI(apiURL, adminToken, `group-2-${Date.now()}`, 'Test group 2')

    // Assign different providers to each group
    await assignProviderToGroupViaAPI(apiURL, adminToken, group1Id, [provider1Id])
    await assignProviderToGroupViaAPI(apiURL, adminToken, group2Id, [provider2Id])

    // Assign user to both groups
    await assignUserToGroupViaAPI(apiURL, adminToken, userId, group1Id)
    await assignUserToGroupViaAPI(apiURL, adminToken, userId, group2Id)

    // Login as test user after creating models/groups so store loads with correct data
    await login(page, baseURL, username, password)
    await goToNewChatPage(page, baseURL)

    await assertModelVisibleInDropdown(page, 'Model X')
    await assertModelVisibleInDropdown(page, 'Model Y')
  })

  test('user with no groups sees empty state in model dropdown', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Ensure admin exists
    await loginAsAdmin(page, baseURL)

    // Create a provider and model (but don't assign to any group)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Unassigned Provider')
    await createModelViaAPI(apiURL, adminToken, providerId, 'model-z', 'Model Z')

    // Create test user (no group assignments)
    const username = `nogroup_${Date.now()}`
    const password = 'TestPass123!'
    await createTestUser(apiURL, adminToken, username, `${username}@test.com`, password)

    // Login as test user (store will load with no models since user has no group access)
    await login(page, baseURL, username, password)
    await goToNewChatPage(page, baseURL)

    // Verify dropdown is empty
    await assertDropdownEmpty(page)
  })

  test('removing user from group removes model access', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Ensure admin exists
    await loginAsAdmin(page, baseURL)

    // Setup
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Provider Removal')
    await createModelViaAPI(apiURL, adminToken, providerId, 'model-rm', 'Model RM')

    const username = `removeuser_${Date.now()}`
    const password = 'TestPass123!'
    const userId = await createTestUser(apiURL, adminToken, username, `${username}@test.com`, password)

    // Create group and assign provider via API
    const groupId = await createGroupViaAPI(apiURL, adminToken, `remove-group-${Date.now()}`, 'Removal test group')
    await assignProviderToGroupViaAPI(apiURL, adminToken, groupId, [providerId])

    // Assign user to group
    await assignUserToGroupViaAPI(apiURL, adminToken, userId, groupId)

    // Verify user can see the model (login after setup so store loads with correct data)
    await login(page, baseURL, username, password)
    await goToNewChatPage(page, baseURL)
    await assertModelVisibleInDropdown(page, 'Model RM')

    // Remove user from group via API
    await removeUserFromGroupViaAPI(apiURL, adminToken, userId, groupId)

    // Verify user can no longer see the model (login again after removal so store refreshes)
    await login(page, baseURL, username, password)
    await goToNewChatPage(page, baseURL)
    await assertDropdownEmpty(page)
  })

  test('disabled models do not appear in dropdown', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Ensure admin exists
    await loginAsAdmin(page, baseURL)

    // Setup
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Provider Disabled')

    // Create enabled model
    await createModelViaAPI(apiURL, adminToken, providerId, 'model-enabled', 'Model Enabled', true)

    // Create disabled model
    await createModelViaAPI(apiURL, adminToken, providerId, 'model-disabled', 'Model Disabled', false)

    // Create test user with access to provider
    const username = `disabledtest_${Date.now()}`
    const password = 'TestPass123!'
    const userId = await createTestUser(apiURL, adminToken, username, `${username}@test.com`, password)

    // Create group and assign provider via API
    const groupId = await createGroupViaAPI(apiURL, adminToken, `disabled-group-${Date.now()}`, 'Disabled test group')
    await assignProviderToGroupViaAPI(apiURL, adminToken, groupId, [providerId])
    await assignUserToGroupViaAPI(apiURL, adminToken, userId, groupId)

    // Verify user only sees enabled model (login after setup so store loads with correct data)
    await login(page, baseURL, username, password)
    await goToNewChatPage(page, baseURL)

    await assertModelVisibleInDropdown(page, 'Model Enabled')
    await assertModelNotVisibleInDropdown(page, 'Model Disabled')
  })
})

// =====================================================
// API Helper Functions
// =====================================================

async function getAdminToken(apiURL: string): Promise<string> {
  const response = await fetch(`${apiURL}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      username: 'admin',
      password: 'password123',
    }),
  })

  if (!response.ok) {
    throw new Error(`Failed to get admin token: ${response.statusText}`)
  }

  const data = await response.json()
  return data.access_token
}

async function createProviderViaAPI(
  apiURL: string,
  token: string,
  name: string
): Promise<string> {
  const response = await fetch(`${apiURL}/api/llm-providers`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      name,
      provider_type: 'local',
      enabled: true,
    }),
  })

  if (!response.ok) {
    throw new Error(`Failed to create provider: ${response.statusText}`)
  }

  const data = await response.json()
  return data.id
}

async function createModelViaAPI(
  apiURL: string,
  token: string,
  providerId: string,
  name: string,
  displayName: string,
  enabled: boolean = true
): Promise<string> {
  const response = await fetch(`${apiURL}/api/llm-models`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      provider_id: providerId,
      name,
      display_name: displayName,
      enabled,
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
    const text = await response.text()
    throw new Error(`Failed to create model: ${response.statusText} - ${text}`)
  }

  const data = await response.json()
  return data.id
}

async function createGroupViaAPI(
  apiURL: string,
  token: string,
  name: string,
  description: string
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
      permissions: [],
    }),
  })

  if (!response.ok) {
    const text = await response.text()
    throw new Error(`Failed to create group: ${response.statusText} - ${text}`)
  }

  const data = await response.json()
  return data.id
}

async function assignUserToGroupViaAPI(
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
    const text = await response.text()
    throw new Error(`Failed to assign user to group: ${response.statusText} - ${text}`)
  }
}

async function assignProviderToGroupViaAPI(
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
    const text = await response.text()
    throw new Error(`Failed to assign provider to group: ${response.statusText} - ${text}`)
  }
}

async function removeUserFromGroupViaAPI(
  apiURL: string,
  token: string,
  userId: string,
  groupId: string
): Promise<void> {
  const response = await fetch(`${apiURL}/api/groups/${userId}/${groupId}/remove`, {
    method: 'DELETE',
    headers: {
      Authorization: `Bearer ${token}`,
    },
  })

  if (!response.ok) {
    const text = await response.text()
    throw new Error(`Failed to remove user from group: ${response.statusText} - ${text}`)
  }
}
