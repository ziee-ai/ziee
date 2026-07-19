import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  type AgentActivityEntry,
  TOOL_ACTIVITY_PHRASES,
  describeActivity,
  phraseForTool,
  titleCaseToolId,
} from './activityDescriptors.ts'

// TEST-18 — the activity descriptor registry + `describeActivity`. Known tool
// ids map to domain-language phrases; an unknown tool gets a sensible
// title-cased fallback; a backend-provided `title` always wins.

const entry = (over: Partial<AgentActivityEntry>): AgentActivityEntry =>
  ({
    type: 'agent_activity',
    title: '',
    kind: 'tool_call',
    seq: 1,
    status: 'running',
    tool: null,
    ...over,
  }) as AgentActivityEntry

test('registry maps known tool ids to domain-language phrases', () => {
  assert.equal(TOOL_ACTIVITY_PHRASES.web_search, 'Searching the web')
  assert.equal(phraseForTool('web_search'), 'Searching the web')
  assert.equal(phraseForTool('literature_search'), 'Searching the literature')
  assert.equal(phraseForTool('code_sandbox'), 'Running code')
  assert.equal(phraseForTool('execute_command'), 'Running code')
  assert.equal(phraseForTool('search_knowledge'), 'Searching your knowledge base')
})

test('titleCaseToolId title-cases an unregistered id', () => {
  assert.equal(titleCaseToolId('fetch_paper_fulltext'), 'Fetch Paper Fulltext')
  assert.equal(titleCaseToolId('some-weird.tool_id'), 'Some Weird Tool Id')
})

test('phraseForTool falls back to a title-cased id for an unknown tool', () => {
  assert.equal(phraseForTool('my_custom_tool'), 'My Custom Tool')
})

test('phraseForTool: blank/undefined tool -> generic Working…', () => {
  assert.equal(phraseForTool(''), 'Working…')
  assert.equal(phraseForTool('   '), 'Working…')
  assert.equal(phraseForTool(null), 'Working…')
  assert.equal(phraseForTool(undefined), 'Working…')
})

test('describeActivity PREFERS a backend-provided title over the tool phrase', () => {
  const e = entry({ title: 'Cross-checking dosages', tool: 'web_search' })
  assert.equal(describeActivity(e), 'Cross-checking dosages')
})

test('describeActivity: blank title -> registry phrase for the tool', () => {
  assert.equal(describeActivity(entry({ title: '', tool: 'web_search' })), 'Searching the web')
  // whitespace-only title is treated as blank
  assert.equal(describeActivity(entry({ title: '   ', tool: 'code_sandbox' })), 'Running code')
})

test('describeActivity: blank title + unknown tool -> title-cased fallback', () => {
  assert.equal(describeActivity(entry({ title: '', tool: 'run_forecast' })), 'Run Forecast')
})

test('describeActivity: no title, no tool -> Working…', () => {
  assert.equal(describeActivity(entry({ title: '', tool: null })), 'Working…')
})
