import { Page } from '@playwright/test'
import { createUserGroup, assignProviderToGroup } from '../../05-llm/helpers/group-provider-helpers'
import { assignUserToGroups } from '../../02-users/helpers/user-actions'
import { createProvider } from '../../05-llm/helpers/provider-helpers'

/**
 * Model Access Control Helpers
 *
 * Helpers for setting up and testing user access to LLM models via group assignments
 *
 * NOTE: These helpers use the API to create models directly instead of UI workflows
 * because the UI model creation requires file uploads/downloads which are complex.
 */

// =====================================================
// API-Based Model Creation
// =====================================================

/**
 * Create a simple test model using the API directly
 * This bypasses the complex UI file upload/download flows
 */
export async function createModelViaAPI(
  _page: Page,
  _providerName: string,
  _modelDisplayName: string
): Promise<void> {
  // We'll use the API client to create a model
  // First, we need to get the provider ID by navigating and extracting it
  // For now, we'll use a simpler approach - just create via API in the test setup

  // NOTE: The actual implementation will be in the test files using
  // the global test fixtures that provide API access
  // This helper is just for documentation purposes
}

/**
 * Simplified access setup using API for model creation
 *
 * This helper creates:
 * 1. Provider (via UI)
 * 2. Group (via UI)
 * 3. Model (via API - handled in test setup)
 * 4. Assign provider to group (via UI)
 */
export async function setupProviderAndGroup(
  page: Page,
  baseURL: string,
  providerName: string,
  groupName: string
): Promise<void> {
  // Create provider
  await createProvider(page, baseURL, { name: providerName }, 'local')

  // Create user group
  await createUserGroup(page, baseURL, groupName, `Test group for ${groupName}`)

  // Assign provider to group
  await assignProviderToGroup(page, groupName, providerName)
}

/**
 * Grant a user access to a group
 */
export async function grantUserAccessToGroup(
  page: Page,
  username: string,
  groupName: string
): Promise<void> {
  await assignUserToGroups(page, username, [groupName])
}
