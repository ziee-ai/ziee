import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import { installVoiceBrowserMocks, routeVoice, defaultVoiceState } from './voice-helpers'

/**
 * TEST-12 — the streaming admin fields (streaming_enabled + stream_interval_ms)
 * in VoiceConfigCard: edit, save, and persist across a reload; the cadence input
 * enforces its bounds.
 */
test.describe('Voice — streaming settings admin (TEST-12)', () => {
  test('edit streaming toggle + cadence, save, persist across reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({ timeout: 30000 })

    // Fields render with the seeded defaults.
    const streamingSwitch = byTestId(page, 'voice-config-streaming-enabled')
    const interval = byTestId(page, 'voice-config-stream-interval')
    await expect(streamingSwitch).toBeVisible()
    await expect(interval).toHaveValue('1000')
    // The cadence input carries its bounds (validation guard).
    await expect(interval).toHaveAttribute('min', '300')
    await expect(interval).toHaveAttribute('max', '10000')

    // Turn streaming OFF and set the cadence to 2500ms.
    await streamingSwitch.click()
    await interval.click()
    await interval.fill('2500')

    await byTestId(page, 'voice-config-save-btn').click()
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({ timeout: 10000 })

    // The PUT carried the streaming fields to the backend.
    expect(voice.state.settings.streaming_enabled).toBe(false)
    expect(voice.state.settings.stream_interval_ms).toBe(2500)

    // Reload → persisted.
    await page.reload({ waitUntil: 'load' })
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'voice-config-stream-interval')).toHaveValue('2500')
  })
})
