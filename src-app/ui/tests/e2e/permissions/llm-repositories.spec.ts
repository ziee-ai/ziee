/**
 * Permission-gating E2E for /settings/llm-repositories (audit id all-11b03324b82d).
 * The route + settings entry are gated on Permissions.LlmRepositoriesRead
 * (module.tsx:34,63). A non-admin member WITHOUT that permission must not see
 * the settings menu entry and a deep-link must render the inline 403 (URL
 * preserved), mirroring the auth-providers permission spec.
 *
 * Uses the `no-403` fixture so an accidental /api/* 403 during the member test
 * fails loudly.
 */
import { test, expect } from './no-403'
import { loginAsMember } from './fixtures'
import { byTestId } from '../testid'

test.describe('llm-repositories — permission gating', () => {
  test('member without llm_repositories::read cannot access the repositories page', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // The settings menu entry is hidden for a user without the read perm.
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(
      byTestId(page, 'settings-nav-menu-item-llm-repositories'),
    ).toHaveCount(0)

    // Deep-link → inline 403, URL preserved (not a silent redirect).
    await page.goto(`${testInfra.baseURL}/settings/llm-repositories`)
    await expect(byTestId(page, 'router-route-forbidden-result')).toBeVisible()
    expect(page.url()).toContain('/settings/llm-repositories')
  })
})
