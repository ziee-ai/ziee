import { loginAsAdmin } from '../../common/auth-helpers'
import { expect, test } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  defaultVoiceState,
  installVoiceBrowserMocks,
  mkVoiceModel,
  routeVoice,
} from './voice-helpers'

/**
 * Whisper model management (download / upload / activate / delete) on the
 * reworked /settings/voice page. All /api/voice/** is mocked via voice-helpers
 * so no whisper runtime, DB rows, or network are needed.
 *
 * TEST-17 — AvailableModelsCard lists a paginated catalog; Install shows the
 *           inline SSE progress bar advancing to complete.
 * TEST-18 — the installed model appears; Set-active + Delete (Confirm) work;
 *           the active-model delete guard is honored.
 * TEST-19 — the Upload drawer: select a file, per-file/overall progress render,
 *           and on success the model appears tagged upload/unverified.
 * TEST-20 — at 390px the cards render without horizontal page scroll.
 */
test.describe('Voice — model management', () => {
  test('TEST-17: catalog paginates; Install drives the progress bar to complete', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // The available-models card lists the catalog, paginated (12 > PAGE_SIZE 10).
    const available = byTestId(page, 'voice-available-models-card')
    await expect(available).toBeVisible()
    await expect(
      byTestId(page, 'voice-available-models-pagination'),
    ).toContainText(/of 12/, { timeout: 15000 })
    // A first-page model row + its Install button.
    await expect(byTestId(page, 'voice-available-model-row-base')).toBeVisible()

    // Install `base` → POST download → SSE (connected/progress/complete).
    await byTestId(page, 'voice-available-model-install-base').click()

    // The inline progress line renders (it lingers ~2s after complete before
    // auto-dismiss), proving the progress→complete pipeline ran.
    await expect(
      page.locator('[data-testid^="voice-model-download-progress-"]'),
    ).toBeVisible({ timeout: 15000 })

    // On complete the catalog reload flips `base` to installed.
    await expect(
      byTestId(page, 'voice-available-model-installed-tag-base'),
    ).toBeVisible({ timeout: 15000 })
  })

  test('TEST-18: installed model appears, set-active + delete work, active-delete guard honored', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // Install `base` from the catalog → it lands in the installed library.
    await byTestId(page, 'voice-available-model-install-base').click()
    await expect(byTestId(page, 'voice-installed-model-row-base')).toBeVisible({
      timeout: 15000,
    })

    // Set it active → the active tag appears and the set-active button drops.
    await byTestId(page, 'voice-installed-model-activate-base').click()
    await expect(
      byTestId(page, 'voice-installed-model-active-tag-base'),
    ).toBeVisible({ timeout: 10000 })
    await expect(
      byTestId(page, 'voice-installed-model-activate-base'),
    ).toHaveCount(0)

    // Delete the ACTIVE model → the guard requires acknowledging first.
    await byTestId(page, 'voice-installed-model-delete-base').click()
    const confirmOk = byTestId(
      page,
      'voice-installed-model-delete-confirm-base-confirm',
    )
    const ack = byTestId(page, 'voice-installed-model-delete-ackactive-base')
    await expect(ack).toBeVisible({ timeout: 10000 })
    // Guard honored: OK is disabled until the active-model ack is checked.
    await expect(confirmOk).toBeDisabled()
    await ack.click()
    await expect(confirmOk).toBeEnabled()
    await confirmOk.click()

    // The row is removed.
    await expect(byTestId(page, 'voice-installed-model-row-base')).toHaveCount(
      0,
      { timeout: 10000 },
    )
  })

  test('TEST-19: upload drawer shows progress and the uploaded model appears tagged upload/unverified', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // Open the upload drawer.
    await byTestId(page, 'voice-model-upload-open-btn').click()
    await expect(byTestId(page, 'voice-upload-drawer-submit-btn')).toBeVisible({
      timeout: 10000,
    })

    // Select a ggml file → the name auto-derives (ggml-myupload.bin → myupload).
    await byTestId(page, 'voice-upload-files')
      .locator('input[type="file"]')
      .setInputFiles({
        name: 'ggml-myupload.bin',
        mimeType: 'application/octet-stream',
        buffer: Buffer.from('fake ggml model bytes'),
      })
    await expect(byTestId(page, 'voice-upload-selected-file')).toBeVisible()

    // Submit → the upload is held ~1.2s server-side so the progress card shows.
    await byTestId(page, 'voice-upload-drawer-submit-btn').click()
    await expect(byTestId(page, 'voice-upload-progress-card')).toBeVisible({
      timeout: 10000,
    })
    await expect(byTestId(page, 'voice-upload-file-progress-0')).toBeVisible()

    // On success the drawer closes and the uploaded model appears in the library
    // tagged `upload` + `unverified`.
    await expect(
      byTestId(page, 'voice-installed-model-row-myupload'),
    ).toBeVisible({ timeout: 15000 })
    await expect(
      byTestId(page, 'voice-installed-model-source-myupload'),
    ).toContainText('upload')
    await expect(
      byTestId(page, 'voice-installed-model-verified-myupload'),
    ).toContainText('unverified')
  })

  test('TEST-20: at 390px the cards render without horizontal page scroll', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await page.setViewportSize({ width: 390, height: 844 })
    await installVoiceBrowserMocks(page)
    await routeVoice(
      page,
      defaultVoiceState({
        models: [mkVoiceModel('base', { is_active: true })],
      }),
    )

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // Both reworked cards render at mobile width.
    await expect(byTestId(page, 'voice-available-models-card')).toBeVisible()
    await expect(byTestId(page, 'voice-installed-models-card')).toBeVisible()

    // No horizontal page scroll (controls wrap rather than overflow the body).
    const overflow = await page.evaluate(() => {
      const el = document.documentElement
      return el.scrollWidth - el.clientWidth
    })
    expect(overflow).toBeLessThanOrEqual(1)
  })
})
