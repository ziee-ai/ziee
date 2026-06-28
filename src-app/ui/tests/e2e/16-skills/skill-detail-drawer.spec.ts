import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'

/**
 * E2E — clicking a skill card opens the SkillDetailDrawer.
 *
 * Audit gap (all-87083ad3eae2): `list-page-renders.spec.ts` only asserts
 * the list renders; nothing exercised the card → drawer interaction.
 * Each card in `SkillsList.tsx` is `role="button"` (+ `data-skill-id`) and
 * its onClick calls `Stores.SkillDrawer.open(skill)`, opening
 * `SkillDetailDrawer` — which renders a Descriptions table (Name / Files /
 * Size) plus the skill title, and fetches the SKILL.md body.
 *
 * ziee's built-in capability skills are boot-synced into every fresh DB,
 * so /skills is never empty — we click the first built-in card and assert
 * the drawer opens. Fully deterministic (no real LLM).
 */

test.describe('Skills — detail drawer interaction', () => {
  test('clicking a skill card opens the detail drawer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSkillsPage(page, baseURL)

    // The boot-synced built-in skills render as clickable cards.
    const firstCard = page.locator('[data-skill-id]').first()
    await expect(firstCard).toBeVisible({ timeout: 15000 })
    // Capture the card's title text so we can assert it reappears in the
    // drawer header (the drawer title repeats display_name || name).
    const cardTitle = (await firstCard.locator('strong').first().innerText()).trim()
    await firstCard.click()

    // The drawer is an antd Drawer (role="dialog"); its body shows a
    // Descriptions table with a stable "Files" row label and the skill
    // title in the header.
    const drawer = page.getByRole('dialog')
    await expect(drawer).toBeVisible({ timeout: 10000 })
    await expect(drawer.getByText('Files', { exact: true })).toBeVisible()
    await expect(drawer.getByText(cardTitle).first()).toBeVisible()
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

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
    const card = page.locator('.ant-card[role], .ant-card').filter({ hasText: /.+/ }).first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await card.click()

    // The drawer opens and the SKILL.md body resolves (success heading).
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer).toBeVisible({ timeout: 10000 })
    await expect(
      drawer.getByText('Skill content (SKILL.md)'),
    ).toBeVisible({ timeout: 15000 })
    await expect(drawer.getByText("Couldn’t load skill content.")).toHaveCount(0)
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
    const card = page.locator('.ant-card').filter({ hasText: /.+/ }).first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await card.click()

    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer).toBeVisible({ timeout: 10000 })
    await expect(
      drawer.getByText("Couldn’t load skill content."),
    ).toBeVisible({ timeout: 15000 })
  })
})
