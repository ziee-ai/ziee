import type { Page } from '@playwright/test'
import {
  mockChatTokenStream,
  startedEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageContent,
} from '../../helpers/sse-mock-helpers'
import { goToNewChatPage, selectModelInDropdown } from '../../09-chat/helpers/chat-helpers'

// Seed a `literature_search` tool_result with a typed `structured_content`
// payload — the deterministic "no live LLM" path the screening-flow E2E needs.
// Uses the CURRENT fire-and-forget chat helper (`mockChatTokenStream`); the
// persisted tool_result block (carrying `structured_content`) is delivered by
// the post-`complete` /messages reload (`mockGetMessages`), so the card renders
// from the same snake_case shape the production reload produces — not from a
// hand-shaped stream frame. The LiteratureToolResultCard reads
// block.structured_content; "Open in screening" hands those records to the panel.

export interface LitRecord {
  doi?: string | null
  pmid?: string | null
  title: string
  abstract_text?: string | null
  authors: string[]
  year?: number | null
  venue?: string | null
  url?: string | null
  source: string
  source_ids: string[]
  cited_by_count?: number | null
  is_preprint: boolean
  relevance: number
}

export interface LitStructured {
  query: string
  records: LitRecord[]
  identified: Record<string, number>
  after_dedup: number
  degraded_sources: string[]
  completeness: { estimate: string; method: string; caveat: string } | null
}

export async function seedLiteratureResult(
  page: Page,
  baseURL: string,
  sc: LitStructured,
): Promise<string> {
  const toolUseId = `tu_lit_${Math.random().toString(36).slice(2, 9)}`
  const assistantMessageId = `amsg_lit_${Math.random().toString(36).slice(2, 9)}`
  const userMessageId = `umsg_lit_${Math.random().toString(36).slice(2, 9)}`
  const serverId = 'lit-search-test-server'
  const digest = `Literature search: "${sc.query}" — ${sc.after_dedup} after dedup.`

  // Minimal generation frames — the tool_result block itself arrives via the
  // /messages reload below (mockGetMessages), the real production path.
  await mockChatTokenStream(page, [[startedEvent({ userMessageId }), completeEvent()]])

  const toolResult: MockMessageContent = {
    content_type: 'tool_result',
    content: {
      type: 'tool_result',
      tool_use_id: toolUseId,
      name: 'literature_search',
      server_id: serverId,
      content: digest,
      structured_content: sc,
      is_error: false,
    },
  }
  const toolUse: MockMessageContent = {
    content_type: 'tool_use',
    content: { type: 'tool_use', id: toolUseId, name: 'literature_search', server_id: serverId, input: { query: sc.query } },
  }

  await mockGetMessages(page, [
    mockUserMessage({ id: userMessageId, text: 'find papers' }),
    { id: assistantMessageId, role: 'assistant', contents: [toolUse, toolResult] },
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
  const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
  await textarea.fill('find papers')
  await page.getByRole('button', { name: 'Send message' }).click()

  await page
    .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
    .first()
    .waitFor({ state: 'visible', timeout: 15000 })
  return assistantMessageId
}

export function sampleResult(): LitStructured {
  const rec = (doi: string, title: string, year: number): LitRecord => ({
    doi,
    pmid: null,
    title,
    abstract_text: `Abstract for ${title}.`,
    authors: ['Smith J', 'Doe A'],
    year,
    venue: 'Nature',
    url: `https://doi.org/${doi}`,
    source: 'europepmc',
    source_ids: [`europepmc:${doi}`],
    cited_by_count: 10,
    is_preprint: false,
    relevance: 0.9,
  })
  return {
    query: 'CRISPR base editing off-target',
    records: [
      rec('10.1/aaa', 'Base editing reduces off-target effects', 2021),
      rec('10.1/bbb', 'A second relevant study', 2022),
    ],
    identified: { europepmc: 2, crossref: 1 },
    after_dedup: 2,
    degraded_sources: [],
    completeness: { estimate: 'moderate', method: 'cross-source overlap', caveat: 'Heuristic, not a recall rate.' },
  }
}
