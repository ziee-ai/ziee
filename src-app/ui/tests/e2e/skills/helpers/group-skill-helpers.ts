import { type APIRequestContext, type Page, expect } from '@playwright/test'
import { gzipSync } from 'node:zlib'
import { byTestId } from '../../testid'

/**
 * Helpers for the System-Skills ↔ User-group assignment widget/drawer, mirrors
 * `mcp/helpers/group-server-helpers.ts`. The widget testids are id-keyed
 * (`skill-group-widget-card-<gid>` / `-edit-btn-<gid>` / `-tag-<sid>`), the
 * drawer testids too (`skill-group-assign-card-<sid>` / `-switch-<sid>` /
 * `-save-btn` / `-cancel-btn`). Group-page navigation + CRUD are reused from
 * the MCP helpers.
 */

// ---- fixture: seed a system skill via the multipart import API ----

function buildSkillBundle(skillMd: string): Buffer {
  const content = Buffer.from(skillMd, 'utf8')
  const header = Buffer.alloc(512)
  header.write('SKILL.md', 0, 'utf8')
  header.write('0000644\0', 100, 'utf8')
  header.write('0000000\0', 108, 'utf8')
  header.write('0000000\0', 116, 'utf8')
  header.write(content.length.toString(8).padStart(11, '0') + '\0', 124, 'utf8')
  header.write('00000000000\0', 136, 'utf8')
  header.write('0', 156, 'utf8')
  header.write('ustar\0', 257, 'utf8')
  header.write('00', 263, 'utf8')
  for (let i = 148; i < 156; i++) header[i] = 0x20
  let sum = 0
  for (let i = 0; i < 512; i++) sum += header[i]
  header.write(sum.toString(8).padStart(6, '0') + '\0 ', 148, 'utf8')
  const bodyPad = (512 - (content.length % 512)) % 512
  const tar = Buffer.concat([header, content, Buffer.alloc(bodyPad), Buffer.alloc(1024)])
  return gzipSync(tar)
}

/**
 * Install a SYSTEM-scope skill via `POST /api/skills/import` (scope=system).
 * Returns the skill's display name (the frontmatter `name`), which the widget
 * tags + drawer cards render.
 */
export async function seedSystemSkill(
  request: APIRequestContext,
  apiURL: string,
  token: string,
  name: string,
): Promise<string> {
  const skillMd = `---
name: ${name}
description: A throwaway SYSTEM skill seeded by the group-assignment E2E.
---

# ${name}

Body of a valid system-scope SKILL.md.
`
  const resp = await request.post(`${apiURL}/api/skills/import`, {
    headers: { Authorization: `Bearer ${token}` },
    multipart: {
      bundle: {
        name: 'skill.tar.gz',
        mimeType: 'application/gzip',
        buffer: buildSkillBundle(skillMd),
      },
      scope: 'system',
      name,
    },
  })
  expect(resp.status(), `system skill import should 201: ${await resp.text()}`).toBe(201)
  return name
}

// ---- widget + drawer interactions ----

function groupCardByName(page: Page, groupName: string) {
  return page.getByTestId(/^user-group-card-/).filter({ hasText: groupName }).first()
}

export async function openSkillAssignmentDrawerFromGroup(page: Page, groupName: string) {
  const card = groupCardByName(page, groupName)
  await card.waitFor({ state: 'visible', timeout: 10000 })
  await card.scrollIntoViewIfNeeded()
  await card.getByTestId(/^skill-group-widget-edit-btn-/).click()
  await byTestId(page, 'skill-group-assign-save-btn').waitFor({ state: 'visible', timeout: 5000 })
}

export async function toggleSkillInDrawer(page: Page, skillName: string, enable: boolean) {
  const skillCard = page
    .getByTestId(/^skill-group-assign-card-/)
    .filter({ hasText: skillName })
    .first()
  await skillCard.waitFor({ state: 'visible', timeout: 5000 })
  const sw = skillCard.getByTestId(/^skill-group-assign-switch-/)
  const isChecked = (await sw.getAttribute('aria-checked')) === 'true'
  if (isChecked !== enable) {
    await sw.click()
  }
}

/** Read the drawer switch state for a skill (asserting the pre-checked state). */
export async function skillSwitchChecked(page: Page, skillName: string): Promise<boolean> {
  const skillCard = page
    .getByTestId(/^skill-group-assign-card-/)
    .filter({ hasText: skillName })
    .first()
  await skillCard.waitFor({ state: 'visible', timeout: 5000 })
  const sw = skillCard.getByTestId(/^skill-group-assign-switch-/)
  return (await sw.getAttribute('aria-checked')) === 'true'
}

export async function saveSkillAssignment(page: Page) {
  await byTestId(page, 'skill-group-assign-save-btn').click()
  await byTestId(page, 'skill-group-assign-save-btn').waitFor({ state: 'detached', timeout: 10000 })
}

export async function cancelSkillAssignment(page: Page) {
  await byTestId(page, 'skill-group-assign-cancel-btn').click()
  await byTestId(page, 'skill-group-assign-cancel-btn').waitFor({ state: 'detached', timeout: 5000 })
}

export async function assertSkillInGroupWidget(page: Page, groupName: string, skillName: string) {
  const card = groupCardByName(page, groupName)
  const tag = card.getByTestId(/^skill-group-widget-tag-/).filter({ hasText: skillName })
  await expect(tag).toBeVisible()
}

export async function assertSkillNotInGroupWidget(page: Page, groupName: string, skillName: string) {
  const card = groupCardByName(page, groupName)
  const tag = card.getByTestId(/^skill-group-widget-tag-/).filter({ hasText: skillName })
  await expect(tag).toHaveCount(0)
}

export async function assertGroupWidgetShowsSkillCount(
  page: Page,
  groupName: string,
  expectedCount: number,
) {
  const card = groupCardByName(page, groupName)
  await expect(card.getByTestId(/^skill-group-widget-tag-/)).toHaveCount(expectedCount)
}
