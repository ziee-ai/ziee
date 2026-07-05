import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
} from './helpers/chat-helpers'
import { FILE_ASSETS, attachFileViaUI } from './helpers/file-panel-helpers'
import { mockChatStream, startedEvent } from '../helpers/sse-mock-helpers'

/**
 * E2E — ChatRightPanel MOBILE drawer mode (audit gap all-788b166f9359).
 *
 * `chat-right-panel.spec.ts` exercises the panel exclusively at the default
 * (desktop) viewport, where it renders as the resizable side panel keyed by
 * `data-panel-open`. The mobile branch of `ChatRightPanel` (ChatRightPanel.tsx
 * :156-175) is structurally different and was never exercised: when
 * `useWindowMinSize().sm` is true (viewport ≤ 640px) the panel instead renders
 * a FULL-SCREEN fixed overlay — `<div class="fixed inset-0 z-[1000]" role="dialog"
 * aria-modal="true" aria-label="Chat panel">` — and is dismissed via
 * `closeMobileDrawer`, NOT the side-panel collapse. This drives that branch at a
 * 480px viewport: opening a file card must surface the modal drawer overlay (not
 * the side panel), and the close button must tear it down.
 */

async function setupProviderAndModel(apiURL: string, adminToken: string) {
  const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
  await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')
}

async function setupChatAtNewConversation(page: Page, baseURL: string, apiURL: string) {
  await loginAsAdmin(page, baseURL)
  const adminToken = await getAdminToken(apiURL)
  await setupProviderAndModel(apiURL, adminToken)
  // started-only stream: the optimistic user bubble (with its file card) stays
  // mounted for the drawer flow without a real LLM completing the turn. The
  // drawer test operates on the USER message's file card, so no assistant
  // response is needed. Same trick as user-attachments-layout.spec.ts.
  await mockChatStream(page, [[startedEvent({ userMessageId: 'umsg_mobile' })]])
  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
}

test.describe('Chat - Right Panel mobile drawer', () => {
  test('mobile viewport: opening a file renders a full-screen drawer overlay, closeable', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Narrow viewport BEFORE navigating so `useWindowMinSize().sm` (≤ 640px)
    // is true on first render and the panel takes its mobile branch.
    await page.setViewportSize({ width: 480, height: 900 })

    await setupChatAtNewConversation(page, baseURL, apiURL)

    // Attach + send so the user message carries a clickable FileCard.
    const sendButton = byTestId(page, 'chat-input-send-btn')
    await expect(sendButton).toBeEnabled({ timeout: 30000 })
    await attachFileViaUI(page, FILE_ASSETS.md)
    await byTestId(page, 'chat-message-textarea').fill('see attached')
    await expect(sendButton).toBeEnabled({ timeout: 30000 })
    await sendButton.click()

    // The sent user message carries the clickable FileCard (no assistant
    // response required — the started-only stream keeps the bubble mounted).
    await expect(
      page.locator('[data-testid="file-card"][data-filename="test.md"]').last(),
    ).toBeVisible({ timeout: 15000 })

    // Before opening: the mobile drawer (the right panel, role=dialog on mobile)
    // is not present.
    const drawer = byTestId(page, 'chat-right-panel')
    await expect(drawer).toHaveCount(0)

    // Click the most-recent file card to display it in the right panel.
    // `displayInRightPanel` sets `mobileDrawerOpen: true`, which on a mobile
    // viewport mounts the full-screen overlay branch.
    await page
      .locator('[data-testid="file-card"][data-filename="test.md"]')
      .last()
      .click()

    // Mobile branch assertions: the panel renders AS the modal drawer, NOT the
    // desktop side panel (which would expose `data-panel-open` and no role).
    await expect(drawer).toBeVisible({ timeout: 10000 })
    const panel = page.locator('[data-testid="chat-right-panel"]')
    await expect(panel).toHaveAttribute('role', 'dialog')
    await expect(panel).toHaveAttribute('aria-modal', 'true')
    // Full-screen fixed overlay class contract (covers the page incl. header).
    await expect(panel).toHaveClass(/fixed/)
    await expect(panel).toHaveClass(/inset-0/)
    // The desktop side-panel marker must be absent in mobile mode.
    await expect(panel).not.toHaveAttribute('data-panel-open', 'true')
    // The opened tab's content surfaced inside the drawer (tab labelled by the
    // uploaded filename — dynamic data).
    await expect(
      byTestId(page, 'chat-right-panel-tab-list')
        .getByRole('tab')
        .filter({ hasText: 'test.md' }),
    ).toBeVisible()

    // Close the drawer via its close button → `closeMobileDrawer` flips
    // `mobileDrawerOpen` false and the mobile branch returns null (overlay gone).
    await page.locator('[data-testid="chat-right-panel-close"]').click()
    await expect(drawer).toHaveCount(0, { timeout: 5000 })
  })
})
