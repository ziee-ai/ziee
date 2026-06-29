import { Page } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * MCP-specific navigation helpers
 */

// NOTE: do NOT wait for 'networkidle' here. The app opens a persistent
// realtime-sync SSE (`/api/sync/subscribe`) on every authenticated page; an
// open EventSource is an in-flight request that never completes, so
// 'networkidle' never fires and the navigation times out. Wait for 'load'
// and let the per-page `waitForMcp*PageLoad` helpers gate on the actual
// heading + "Add Server" content (which only render after data loads).
export async function goToMcpServersPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/mcp-servers`)
  await page.waitForLoadState('load')
}

export async function goToMcpAdminPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/mcp-admin`)
  await page.waitForLoadState('load')
}

export async function waitForMcpPageLoad(page: Page) {
  // The Add button only renders once the page data has loaded, so gating on
  // its testid covers both the heading-present + not-loading conditions.
  await byTestId(page, 'mcp-settings-add-btn').waitFor({ state: 'visible', timeout: 30000 })
}

export async function waitForMcpAdminPageLoad(page: Page) {
  // The system Add button only renders after the page data loads.
  await byTestId(page, 'mcp-system-add-btn').waitFor({ state: 'visible', timeout: 30000 })
}

export async function clickServerCard(page: Page, serverDisplayName: string, isAdmin: boolean = false) {
  // Cards are keyed by server id (mcp-server-card-<id> on the user page,
  // mcp-system-server-card-<id> on the admin page). Match the card whose
  // content carries the dynamic display name the test created.
  const cardPrefix = isAdmin ? /^mcp-system-server-card-/ : /^mcp-server-card-/
  const serverCard = page
    .getByTestId(cardPrefix)
    .filter({ hasText: serverDisplayName })
    .first()
  await serverCard.waitFor({ state: 'visible', timeout: 10000 })

  // Scroll the server card into view
  await serverCard.scrollIntoViewIfNeeded()

  // Wait for associated cards (like User Groups card) to render
  await page.waitForTimeout(1000)
}

export async function goToServerDetail(
  page: Page,
  baseURL: string,
  serverId: string,
  isAdmin: boolean = false
) {
  const path = isAdmin ? `/settings/mcp-admin/${serverId}` : `/settings/mcp-servers/${serverId}`
  await page.goto(`${baseURL}${path}`)
  await page.waitForLoadState('load')
}
