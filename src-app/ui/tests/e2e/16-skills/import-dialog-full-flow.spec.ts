import { gzipSync } from 'node:zlib'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'

/**
 * E2E — the Import-Skill dialog FULL flow (ImportSkillDialog.tsx).
 *
 * Audit gap (all-d9afd42a1de2): `import-dialog-validate.spec.ts` covers only
 * the Validate → success-Alert half (a dropped SKILL.md). The actual IMPORT
 * branch — dropping a real `.tar.gz` skill bundle into the Dragger and
 * clicking "Import", which POSTs the multipart bundle to
 * `POST /api/skills/import` (handleImport → Stores.Skill.importSkill), creates
 * the `local.dev.<owner>/<slug>` skill row, fires the "Skill imported" toast,
 * closes the dialog, and surfaces the new skill card on `/skills` — had no
 * E2E. This covers that real round-trip (only the file upload is synthetic;
 * the import HTTP round-trip runs for real, no route mocks).
 */

// A beacon name so the imported skill is unambiguously distinguishable from
// the always-present built-in capability skills on the list.
const SKILL_NAME = 'e2e-imported-skill-beacon'

const VALID_SKILL_MD = `---
name: ${SKILL_NAME}
description: A throwaway skill imported by the import-dialog full-flow E2E.
---

# E2E Imported Skill

Body of a valid SKILL.md that the import path installs as a dev skill.
`

/**
 * Build a single-file gzip+tar bundle containing `SKILL.md` at the archive
 * root — the exact layout `import_skill` expects (`entry_point = "SKILL.md"`).
 * Mirrors `17-workflows/helpers/workflow-helpers.ts::buildWorkflowBundle`,
 * emitting just enough of the POSIX ustar format for one small text entry
 * (the backend's bomb-guarded tar reader accepts a standard ustar archive).
 * Runs in Node (test process), so `node:zlib` is available.
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

test.describe('Skills — Import dialog full flow', () => {
  test('drop a bundle → Import → skill is created and appears on the list', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSkillsPage(page, baseURL)

    await page.getByRole('button', { name: /import/i }).click()

    const dialog = page.getByRole('dialog', { name: 'Import Skill' })
    await expect(dialog).toBeVisible()

    // Drop a real tar.gz skill bundle (SKILL.md at root) into the antd
    // Dragger's hidden file input.
    await dialog.locator('input[type="file"]').setInputFiles({
      name: 'skill.tar.gz',
      mimeType: 'application/gzip',
      buffer: buildSkillBundle(VALID_SKILL_MD),
    })

    // Click Import and assert the REAL multipart import round-trip succeeds.
    const importResp = page.waitForResponse(
      r =>
        r.url().includes('/api/skills/import') &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await dialog.getByRole('button', { name: 'Import' }).click()
    expect((await importResp).ok()).toBeTruthy()

    // The dialog closes and the success toast fires.
    await expect(page.getByText('Skill imported')).toBeVisible({
      timeout: 15000,
    })
    await expect(dialog).toBeHidden({ timeout: 15000 })

    // The imported skill surfaces on the list (the card renders
    // `display_name`, which the import derives from the SKILL.md `name`
    // frontmatter).
    await goToSkillsPage(page, baseURL)
    await expect(page.getByText(SKILL_NAME).first()).toBeVisible({
      timeout: 15000,
    })
  })
})
