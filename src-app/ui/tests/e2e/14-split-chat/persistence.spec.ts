import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — layout persistence across reload (TEST-21).
 *
 * The SplitView layout persists to localStorage (`ziee-split-view-v1`; the
 * `?pane=` URL mirroring was dropped per DRIFT-1.9). After opening a split and
 * resizing a divider, a full reload restores both panes and the divider width.
 * No LLM needed.
 */
test.describe('Split chat — layout persistence', () => {
  test('a split + resized divider survives a full page reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Split Persist' },
    })
    const convId = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForLoadState('load')

    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0).toBeVisible({ timeout: 15000 })
    await expect(pane1).toBeVisible({ timeout: 15000 })

    // Capture the DEFAULT pane width first, so we can prove the resize actually
    // diverged from it — otherwise a persistence RESET-to-default would false-green
    // the reload check below (audit MEDIUM, a20f).
    const TOL = 24
    const widthDefault = (await pane0.boundingBox())?.width ?? 0
    expect(widthDefault).toBeGreaterThan(0)

    // Resize the left pane via the keyboard-operable divider (WCAG path). Enough
    // steps to move well beyond the tolerance so the divergence is unambiguous.
    const divider = byTestId(page, 'split-divider-0')
    await divider.focus()
    for (let i = 0; i < 12; i++) await divider.press('ArrowRight')
    const widthBefore = (await pane0.boundingBox())?.width ?? 0
    // The resize genuinely changed the width away from the default — without this
    // the persistence assertion could pass on a silent reset-to-default.
    expect(Math.abs(widthBefore - widthDefault)).toBeGreaterThan(TOL)

    // Full reload — the layout must come back from localStorage.
    await page.reload()
    await page.waitForLoadState('load')

    await expect(byTestId(page, 'chat-pane-0')).toBeVisible({ timeout: 20000 })
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 20000 })
    await expect(byTestId(page, 'split-divider-0')).toBeVisible()

    // The reloaded width matches the RESIZED width (persisted), not the default —
    // i.e. within tolerance of widthBefore AND still diverged from widthDefault.
    const widthAfter =
      (await byTestId(page, 'chat-pane-0').boundingBox())?.width ?? 0
    expect(Math.abs(widthAfter - widthBefore)).toBeLessThan(TOL)
    expect(Math.abs(widthAfter - widthDefault)).toBeGreaterThan(TOL)
  })
})
