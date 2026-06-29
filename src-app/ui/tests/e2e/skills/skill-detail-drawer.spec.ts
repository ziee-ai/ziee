import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — SkillDetailDrawer view + async SKILL.md body loading + error state
 * (SkillDetailDrawer.tsx:81-107, 226-244). Opening an installed skill renders
 * its frontmatter summary immediately and lazily fetches the SKILL.md body
 * (GET /api/skills/{id}/body), showing "Skill content (SKILL.md)" on success and
 * "Couldn't load skill content." on failure.
 */

const SEED_SKILL_HUB_ID = 'io.github.ziee/effective-prompting'

async function installSeedSkill(apiURL: string, token: string) {
  const res = await fetch(`${apiURL}/api/skills/install-from-hub`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ hub_id: SEED_SKILL_HUB_ID }),
  })
  if (!res.ok) throw new Error(`install skill failed: ${res.status}`)
}

test.describe('Skills — detail drawer', () => {
  test('opening a skill loads its SKILL.md body', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await installSeedSkill(apiURL, await getAdminToken(apiURL))

    await page.goto(`${baseURL}/skills`)
    const card = page.locator('[data-testid^="skill-list-card-"]').first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await card.click()

    // The drawer opens and the SKILL.md body resolves (body section renders).
    const drawer = byTestId(page, 'skill-detail-sheet-loaded')
    await expect(drawer).toBeVisible({ timeout: 10000 })
    await expect(byTestId(drawer, 'skill-detail-body')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(drawer, 'skill-detail-body-error')).toHaveCount(0)
  })

  test('a failed SKILL.md fetch shows the error state', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await installSeedSkill(apiURL, await getAdminToken(apiURL))

    // Force the body fetch to fail so the drawer's error branch renders.
    await page.route(/\/api\/skills\/[^/]+\/body$/, async (route, req) => {
      if (req.method() === 'GET') {
        return route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: { message: 'boom' } }),
        })
      }
      return route.fallback()
    })

    await page.goto(`${baseURL}/skills`)
    const card = page.locator('[data-testid^="skill-list-card-"]').first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await card.click()

    const drawer = byTestId(page, 'skill-detail-sheet-loaded')
    await expect(drawer).toBeVisible({ timeout: 10000 })
    await expect(byTestId(drawer, 'skill-detail-body-error')).toBeVisible({
      timeout: 15000,
    })
  })
})
