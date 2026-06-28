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
  })
})
