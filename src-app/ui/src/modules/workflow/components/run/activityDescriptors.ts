import type { ProgressKind } from '@/api-client/types'

/**
 * The `agent_activity` member of the generated {@link ProgressKind} union — one
 * accreting row in an agent step's ACTIVITY TIMELINE. Extracted (not re-typed)
 * so it can never drift from the backend contract.
 */
export type AgentActivityEntry = Extract<ProgressKind, { type: 'agent_activity' }>

/**
 * Registry mapping a tool id → a domain-language activity phrase, phrased for a
 * non-technical life-scientist (present-progressive, no jargon). Pure data — no
 * runtime imports — so this module is trivially unit-testable.
 *
 * The map is the fallback: the backend usually ships a good `title` on the
 * activity, and {@link describeActivity} prefers that. This only supplies a
 * phrase when the title is missing/blank.
 */
export const TOOL_ACTIVITY_PHRASES: Record<string, string> = {
  web_search: 'Searching the web',
  literature_search: 'Searching the literature',
  fetch_url: 'Reading a page',
  fetch_paper_fulltext: 'Reading a paper',
  code_sandbox: 'Running code',
  execute_command: 'Running code',
  search_knowledge: 'Searching your knowledge base',
  remember: 'Checking memory',
  recall: 'Checking memory',
  biomcp: 'Searching biomedical databases',
}

/** Title-case a raw tool id (`fetch_paper_fulltext` → `Fetch Paper Fulltext`)
 *  as a last-resort, still-readable label for an unregistered tool. */
export function titleCaseToolId(tool: string): string {
  return tool
    .split(/[_\-.]+/)
    .filter(Boolean)
    .map(w => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}

/** Domain-language phrase for a tool id: the registry entry, else a title-cased
 *  fallback derived from the id. Empty/blank tool → a generic "Working…". */
export function phraseForTool(tool?: string | null): string {
  const t = (tool ?? '').trim()
  if (!t) return 'Working…'
  return TOOL_ACTIVITY_PHRASES[t] ?? titleCaseToolId(t)
}

/**
 * The display line for one activity entry. Prefers the backend-provided `title`
 * when it's a non-blank string (the backend already writes a good editorial
 * line for most activities), otherwise derives a domain-language phrase from the
 * tool id via {@link phraseForTool}. Pure.
 */
export function describeActivity(entry: AgentActivityEntry): string {
  const title = (entry.title ?? '').trim()
  if (title) return title
  return phraseForTool(entry.tool)
}
