import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  installVoiceBrowserMocks,
  routeVoice,
  defaultVoiceState,
  readyCapability,
} from './voice-helpers'

/**
 * TEST-36 — First-run / unprovisioned admin state.
 *
 * Voice is enabled but has no runtime + no model → the "enabled but not ready"
 * banner shows, the installed-runtimes card is empty, and the model reads
 * "not downloaded". After a mock install + model download the banner clears and
 * the instance can be brought to "running".
 */
test.describe('Voice — admin empty state (TEST-36)', () => {
  test('unprovisioned → banner + empty states; provisioning clears them', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const now = new Date().toISOString()
    await installVoiceBrowserMocks(page)
    await routeVoice(
      page,
      defaultVoiceState({
        capability: readyCapability({
          runtime_ready: false,
          model_ready: false,
          can_transcribe: false,
        }),
        versions: [],
        modelStatus: { model: 'base', present: false },
        instance: {
          status: 'stopped',
          state: 'stopped',
          restart_attempts: 0,
          state_changed_at: now,
        },
        updateCheck: {
          platform: 'linux',
          arch: 'x86_64',
          versions: [
            {
              version: 'v1.1.0',
              available_backends: ['cpu'],
              installed_backends: [],
              binary_ready: true,
              installed: false,
              prerelease: false,
              recommended_backend: 'cpu',
              size_bytes: 42 * 1024 * 1024,
              published_at: now,
            },
          ],
        },
      }),
    )

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // ── Unprovisioned posture. ──
    await expect(byTestId(page, 'voice-not-ready-banner')).toBeVisible()
    await expect(byTestId(page, 'voice-installed-empty')).toBeVisible()
    await expect(
      byTestId(byTestId(page, 'voice-model-card'), 'voice-model-missing-tag'),
    ).toBeVisible()
    await expect(
      byTestId(byTestId(page, 'voice-instance-card'), 'voice-instance-state-tag'),
    ).toContainText('stopped')

    // ── Install a runtime (SSE progress→complete adds it). ──
    await byTestId(page, 'voice-version-install-v1.1.0').click()
    await expect(byTestId(page, 'voice-version-desc-v1.1.0')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'voice-installed-empty')).toHaveCount(0)

    // ── Download the model. ──
    await byTestId(page, 'voice-model-download-btn').click()
    await expect(
      byTestId(byTestId(page, 'voice-model-card'), 'voice-model-present-tag'),
    ).toBeVisible({ timeout: 10000 })

    // ── Banner clears once both runtime + model are present. ──
    await expect(byTestId(page, 'voice-not-ready-banner')).toHaveCount(0, {
      timeout: 10000,
    })

    // ── Bring the instance up. ──
    await byTestId(page, 'voice-instance-restart-btn').click()
    await expect(
      byTestId(byTestId(page, 'voice-instance-card'), 'voice-instance-status-tag'),
    ).toContainText('running', { timeout: 10000 })
  })
})
