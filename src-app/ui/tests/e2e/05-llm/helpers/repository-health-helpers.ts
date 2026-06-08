import { Page } from '@playwright/test'

/**
 * Shared seeding + locator helpers for the LLM repository
 * connection-health specs. Kept thin — each spec owns its own mock
 * + page interactions so failure context is local.
 */

export function uniqueRepoName(): string {
  return `health-test-${Math.random().toString(36).slice(2, 10)}`
}

/**
 * Seed a disabled user-scoped repository pointing at the mock URL.
 * Skips the UI to keep the spec focused on the *enable-transition*
 * (or boot) behavior, not the create form.
 *
 * Returns the new row's UUID.
 */
export async function seedRepository(
  apiURL: string,
  token: string,
  name: string,
  mockUrl: string,
): Promise<string> {
  const response = await fetch(`${apiURL}/api/llm-repositories`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      name,
      url: mockUrl,
      auth_type: 'none',
      enabled: false, // start disabled so the spec tests the transition
    }),
  })
  if (!response.ok) {
    throw new Error(
      `Seed failed: ${response.status} ${await response.text()}`,
    )
  }
  const body = await response.json()
  // Response is `LlmRepositoryWithHealthWarning` — pull the row id off
  // its `repository` field.
  return body.repository.id as string
}

/** Locate the repository row by name on the settings page. */
export function repoRow(page: Page, name: string) {
  return page
    .locator('div')
    .filter({ hasText: new RegExp(`^${name}`) })
    .first()
}
