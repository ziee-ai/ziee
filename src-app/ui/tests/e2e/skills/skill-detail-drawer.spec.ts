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

    await page.goto(`${baseURL}/settings/skills`)
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

  // TEST-10 — the drawer now routes its markdown through the shared
  // `preprocessMarkdown`, so it gains all three of that helper's passes. This
  // asserts the one this feature added (LaTeX math) AND the two it inherits
  // (reference-link inlining, blocked-image placeholder) actually render here —
  // the DEC-7 obligation to prove the broadening beyond issue #177 by running it.
  test('renders LaTeX math, inlined reference links and blocked images', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await installSeedSkill(apiURL, await getAdminToken(apiURL))

    await page.route(/\/api\/skills\/[^/]+\/body$/, async (route, req) => {
      if (req.method() !== 'GET') return route.fallback()
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          body:
            'Diffusion at steady state:\n\n' +
            '\\[ \\frac{d^2C(x)}{dx^2} = 0 \\]\n\n' +
            'with inline \\( x^2 \\) too.\n\n' +
            'See the [docs][1] for details.\n\n' +
            '![chart](https://external.example/chart.png)\n\n' +
            '[1]: https://example.com/guide\n',
        }),
      })
    })

    await page.goto(`${baseURL}/settings/skills`)
    const card = page.locator('[data-testid^="skill-list-card-"]').first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await card.click()

    const bodyEl = byTestId(page, 'skill-detail-body')
    await expect(bodyEl).toBeVisible({ timeout: 15000 })

    // (a) math — one display block + one inline, and no raw LaTeX left over
    await expect(bodyEl.locator('.katex-display').first()).toBeVisible({
      timeout: 10000,
    })
    expect(
      await bodyEl.evaluate(el => el.querySelectorAll('.katex').length),
    ).toBe(2)
    await expect(bodyEl).not.toContainText('\\frac{d^2C(x)}{dx^2}')

    // (b) inherited: a cross-block reference link resolves to a real anchor
    await expect(
      bodyEl.locator('a[href="https://example.com/guide"]'),
    ).toHaveText('docs')

    // (c) inherited: the external image degrades to the 🖼 placeholder link
    // rather than a broken, never-loading <img> with a dangling caption
    await expect(bodyEl.locator('img')).toHaveCount(0)
    await expect(bodyEl).toContainText('🖼 chart')
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

    await page.goto(`${baseURL}/settings/skills`)
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
