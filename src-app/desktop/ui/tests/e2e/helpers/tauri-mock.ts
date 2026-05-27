/**
 * Tauri mock — install BEFORE the SPA loads so window.__TAURI__ and
 * window.__TAURI_INTERNALS__.invoke are wired up before any module
 * initialisation runs. Without this, desktop-base/module.tsx sees a
 * web context and skips the auto-login path entirely, and our specs
 * would just be testing the core web behaviour.
 *
 * @tauri-apps/api/core's `invoke()` resolves through
 * window.__TAURI_INTERNALS__.invoke; mocking that one symbol intercepts
 * every invoke call without monkey-patching the npm package.
 */

import type { Page } from '@playwright/test'

export interface AutoLoginTokens {
  user: {
    id: string
    username: string
    email: string
    email_verified: boolean
    is_active: boolean
    is_admin: boolean
    permissions: string[]
    completed_onboarding_ids: string[]
    completed_onboarding_step_ids: string[]
    created_at: string
    updated_at: string
  }
  access_token: string
  refresh_token: string
  expires_in: number
}

export const FAKE_TOKENS: AutoLoginTokens = {
  user: {
    id: '00000000-0000-0000-0000-000000000001',
    username: 'admin',
    email: 'admin@localhost',
    email_verified: true,
    is_active: true,
    is_admin: true,
    permissions: ['*'],
    completed_onboarding_ids: ['getting-started'],
    completed_onboarding_step_ids: [],
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  access_token: 'fake.access.token',
  refresh_token: 'fake.refresh.token',
  expires_in: 3600,
}

export interface InstallTauriMockOptions {
  /** Tokens to return from a successful auto_login. Defaults to FAKE_TOKENS. */
  tokens?: AutoLoginTokens
  /**
   * Strategy for the auto_login mock:
   *   - 'success' (default): resolve with `tokens` immediately
   *   - 'fail-forever': every call rejects with "Server not ready…"
   *   - { failFirstN }: first N calls reject, then success
   */
  autoLogin?: 'success' | 'fail-forever' | { failFirstN: number }
}

export async function installTauriMock(
  page: Page,
  options: InstallTauriMockOptions = {},
): Promise<void> {
  const tokens = options.tokens ?? FAKE_TOKENS
  const autoLogin = options.autoLogin ?? 'success'

  await page.addInitScript(
    ({ tokens, autoLogin }) => {
      // Truthy marker the platform.ts isTauriView check reads.
      ;(window as any).__TAURI__ = { __mocked: true }

      // Track invoke calls so specs can assert retry behaviour.
      ;(window as any).__TAURI_MOCK_CALLS__ = {
        auto_login: 0,
        get_server_port: 0,
      }

      ;(window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, _args?: unknown) => {
          const calls = (window as any).__TAURI_MOCK_CALLS__
          calls[cmd] = (calls[cmd] ?? 0) + 1

          if (cmd === 'auto_login') {
            const n = calls.auto_login
            if (autoLogin === 'fail-forever') {
              throw new Error(
                'Server not ready - JWT service not initialized',
              )
            }
            if (
              typeof autoLogin === 'object' &&
              autoLogin !== null &&
              'failFirstN' in autoLogin &&
              n <= (autoLogin as { failFirstN: number }).failFirstN
            ) {
              throw new Error(
                'Server not ready - JWT service not initialized',
              )
            }
            return tokens
          }
          if (cmd === 'get_server_port') {
            return 8080
          }
          // Unmocked Tauri commands resolve to undefined rather than
          // throw, so unrelated desktop modules (window controls, etc.)
          // don't crash specs that only care about auto-login.
          return undefined
        },
      }
    },
    { tokens, autoLogin },
  )
}

/**
 * Block backend /api/* calls so specs don't hang on a real backend
 * being absent. Routes whose contracts are exercised by individual
 * specs override this with a more specific `page.route` AFTER calling
 * `mockBackendDefaults`.
 */
export async function mockBackendDefaults(page: Page): Promise<void> {
  // Default setup status: no setup needed (matches what the desktop
  // server's ensure_desktop_admin guarantees in production).
  await page.route('**/api/app/setup/status', async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ needs_setup: false }),
    })
  })

  // Catch-all for any other /api call: return an empty 200 so the SPA
  // doesn't surface error boundaries for unrelated startup fetches.
  await page.route('**/api/**', async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: '{}',
    })
  })
}
