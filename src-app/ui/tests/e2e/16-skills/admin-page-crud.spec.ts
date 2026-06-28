import { gzipSync } from 'node:zlib'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToAdminSkillsPage } from './helpers/skill-helpers'

/**
 * E2E — the System Skills admin page CRUD interaction (AdminSkillsPage.tsx).
 *
 * Audit gap (all-ffa56fc95690): `admin-page-gating.spec.ts` only asserts the
 * empty state + the permission gate; it never drives the page's actual CRUD.
 * This covers the real admin lifecycle end-to-end with no route mocks:
 *
 *   CREATE — the admin Import dialog (rendered with `system` so the multipart
 *            POST /api/skills/import carries `scope=system`) installs a
 *            system-scope skill, which surfaces as a card on the admin page;
 *   READ   — clicking that card opens the SkillDetailDrawer;
 *   DELETE — the drawer's Delete affordance (editable because the admin holds
 *            `skills::manage_system`) fires DELETE /api/skills/system/{id} and
 *            the skill leaves the list (empty state returns).
 *
 * Only the uploaded bundle bytes are synthetic; every HTTP round-trip is real.
 */

// Beacon name so the installed system skill is unambiguous on the list.
const SKILL_NAME = 'e2e-admin-system-skill-beacon'

const VALID_SKILL_MD = `---
name: ${SKILL_NAME}
description: A throwaway SYSTEM skill installed by the admin-page CRUD E2E.
---

# E2E Admin System Skill

Body of a valid SKILL.md installed as a system-scope dev skill.
`

/**
 * Build a single-file gzip+tar bundle with `SKILL.md` at the archive root —
 * the layout `import_skill` expects. Mirrors the proven
 * `import-dialog-full-flow.spec.ts::buildSkillBundle` ustar emitter.
 */
function buildSkillBundle(skillMd: string): Buffer {
  const content = Buffer.from(skillMd, 'utf8')
  const header = Buffer.alloc(512)
  header.write('SKILL.md', 0, 'utf8') // name
  header.write('0000644\0', 100, 'utf8') // mode
  header.write('0000000\0', 108, 'utf8') // uid
  header.write('0000000\0', 116, 'utf8') // gid
  header.write(content.length.toString(8).padStart(11, '0') + '\0', 124, 'utf8') // size (octal)
  header.write('00000000000\0', 136, 'utf8') // mtime
  header.write('0', 156, 'utf8') // typeflag (regular file)
  header.write('ustar\0', 257, 'utf8') // magic
  header.write('00', 263, 'utf8') // version
  // checksum: sum of all bytes with the checksum field treated as 8 spaces.
  for (let i = 148; i < 156; i++) header[i] = 0x20
  let sum = 0
  for (let i = 0; i < 512; i++) sum += header[i]
  header.write(sum.toString(8).padStart(6, '0') + '\0 ', 148, 'utf8')

  const bodyPad = (512 - (content.length % 512)) % 512
  const tar = Buffer.concat([
    header,
    content,
    Buffer.alloc(bodyPad),
    Buffer.alloc(1024), // two zero blocks = end-of-archive
  ])
  return gzipSync(tar)
}

test.describe('Skills — Admin page CRUD', () => {
  test('admin imports a system skill then deletes it via the detail drawer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToAdminSkillsPage(page, baseURL)

    // Fresh DB → empty system-skills list to start.
    await expect(
      page.getByText(/no system skills installed/i),
    ).toBeVisible()

    // ---------------------------- CREATE ----------------------------
    // The admin "Import" button (gated by skills::manage_system) opens the
    // ImportSkillDialog rendered with `system`, so handleImport posts the
    // multipart bundle with scope=system.
    await page.getByRole('button', { name: /import/i }).click()

    const dialog = page.getByRole('dialog', { name: 'Import Skill' })
    await expect(dialog).toBeVisible()

    await dialog.locator('input[type="file"]').setInputFiles({
      name: 'system-skill.tar.gz',
      mimeType: 'application/gzip',
      buffer: buildSkillBundle(VALID_SKILL_MD),
    })

    const importResp = page.waitForResponse(
      r =>
        r.url().includes('/api/skills/import') &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await dialog.getByRole('button', { name: 'Import' }).click()
    const imported = await importResp
    expect(imported.ok()).toBeTruthy()

    await expect(page.getByText('Skill imported')).toBeVisible({
      timeout: 15000,
    })
    await expect(dialog).toBeHidden({ timeout: 15000 })

    // The installed system skill surfaces as a card on the admin page.
    const skillCard = page
      .locator('[data-skill-id]')
      .filter({ hasText: SKILL_NAME })
    await expect(skillCard).toBeVisible({ timeout: 15000 })

    // ----------------------------- READ -----------------------------
    // Clicking the card body opens the SkillDetailDrawer.
    await skillCard.getByText(SKILL_NAME).click()
    const drawer = page.getByRole('dialog', { name: SKILL_NAME })
    await expect(drawer).toBeVisible({ timeout: 10000 })

    // ---------------------------- DELETE ----------------------------
    // The drawer's Delete affordance is rendered because a system skill is
    // editable for an admin holding skills::manage_system. Confirm the
    // Popconfirm and assert the real DELETE /api/skills/system/{id} fires.
    await drawer.getByRole('button', { name: /delete/i }).click()
    const deleteResp = page.waitForResponse(
      r =>
        /\/api\/skills\/system\/[^/]+$/.test(r.url()) &&
        r.request().method() === 'DELETE',
      { timeout: 15000 },
    )
    // The Popconfirm's danger "Delete" confirm button (distinct from the
    // small trigger button) — the confirm popover's primary action.
    await page
      .locator('.ant-popconfirm')
      .getByRole('button', { name: 'Delete' })
      .click()
    expect((await deleteResp).ok()).toBeTruthy()

    await expect(page.getByText('Skill deleted')).toBeVisible({
      timeout: 15000,
    })

    // The skill is gone from the admin list — the empty state returns.
    await expect(skillCard).toHaveCount(0, { timeout: 15000 })
    await expect(
      page.getByText(/no system skills installed/i),
    ).toBeVisible({ timeout: 15000 })
  })
})
