/**
 * Skill fixtures for the Skills-in-conversation overlay entries. Seeds the
 * install list (`Stores.Skill.skills`) + the per-conversation available set
 * (`Stores.ConversationSkills.available[cid]`) so the "Skills in this
 * conversation" dialog renders POPULATED (some toggled off), EMPTY, and LOADING.
 */
import type { AvailableSkillEntry, Skill } from '@/api-client/types'

export const SKILLS_CONVERSATION_ID = 'conv-skills-0001'

const base = {
  bundle_sha256: 'a'.repeat(64),
  bundle_size_bytes: 24_576,
  created_at: '2026-02-01T10:00:00Z',
  updated_at: '2026-02-01T10:00:00Z',
  entry_point: 'SKILL.md',
  extracted_path: '/home/user/.ziee/skills/example',
  file_count: 4,
  frontmatter_json: {},
  is_dev: false,
  tags: [],
  enabled: true,
} satisfies Partial<Skill>

/** The user's installed skills (the union rendered as rows). */
export const skillsList: Skill[] = [
  {
    ...base,
    id: 'skill-pdf-0001',
    name: 'com.ziee.pdf-forms',
    display_name: 'PDF form filling',
    description:
      'Fill, flatten, and extract fields from PDF forms using pdftk and pypdf.',
    when_to_use: 'When the user asks to fill out or read a PDF form.',
    scope: 'user',
    version: '1.2.0',
  },
  {
    ...base,
    id: 'skill-xlsx-0002',
    name: 'com.ziee.spreadsheets',
    display_name: 'Spreadsheet analysis',
    description:
      'Read, transform, and chart .xlsx / .csv data with pandas and openpyxl.',
    when_to_use: 'When the user attaches a spreadsheet or asks for a chart.',
    scope: 'system',
    version: '2.0.1',
  },
  {
    ...base,
    id: 'skill-brand-0003',
    name: 'com.ziee.brand-guidelines',
    display_name: 'Brand guidelines',
    description: 'Apply the house style guide to generated documents and decks.',
    scope: 'built_in',
    file_count: 12,
    bundle_size_bytes: 131_072,
  },
]

/** Effective available set for the conversation — the third skill is HIDDEN
 *  (absent here), so its row renders toggled-OFF. */
export const skillsAvailable: AvailableSkillEntry[] = [
  {
    id: 'skill-pdf-0001',
    name: 'com.ziee.pdf-forms',
    description: skillsList[0].description,
    when_to_use: skillsList[0].when_to_use,
  },
  {
    id: 'skill-xlsx-0002',
    name: 'com.ziee.spreadsheets',
    description: skillsList[1].description,
    when_to_use: skillsList[1].when_to_use,
  },
]
