import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import {
  recordKey,
  type LiteratureRecord,
  type LiteratureScreeningData,
  type ScreeningDecision,
} from './types'

interface AiDecision {
  id?: string
  decision?: string
  reason?: string
  confidence?: number
}

/**
 * Bridge an SR workflow run into the literature screening panel — reading the
 * run's deduped candidate set (records + PRISMA counts) and `ai_screening`
 * (per-record first-pass decisions), seeding include/exclude from the AI pass so
 * the human refines from there. Works for the candidate-producing step of every
 * SR workflow: `dedup_all` (the durable `sr-review`), `search` (`sr-search-screen`),
 * or `snowball` (`sr-snowball-screen`). Returns false when the run has no
 * screening-shaped output (so the caller can surface a hint instead of an empty
 * panel).
 */
type CandidateOutput = {
  records?: LiteratureRecord[]
  query?: string
  identified?: Record<string, number>
  after_dedup?: number
  degraded_sources?: string[]
  completeness?: LiteratureScreeningData['completeness']
}

export async function openWorkflowScreening(runId: string): Promise<boolean> {
  const run = await ApiClient.Workflow.getRun({ run_id: runId })

  // `final_output_json` only carries 500-char per-output PREVIEWS (it backs the
  // chat-summary path), so the full record list is NOT reachable there. The
  // complete step outputs live on disk, served by `readOutput`. The candidate set
  // is produced by `dedup_all` (sr-review), `search` (sr-search-screen), or
  // `snowball` (sr-snowball-screen); the AI first-pass decisions by `screen`.
  const stepOutputs = (run?.step_outputs_json ?? {}) as Record<string, unknown>
  const candStep = ['dedup_all', 'search', 'snowball'].find(s => s in stepOutputs)
  if (!candStep || !('screen' in stepOutputs)) return false

  // The screening panel renders inside ChatRightPanel, which only exists in a
  // chat conversation view (and the snapshot persists keyed to the CURRENT
  // conversation). If we're not in a conversation (e.g. opened from
  // /settings/workflows), displaying would set state nothing renders and would
  // not persist — bail so the caller shows a hint instead.
  if (!Stores.Chat.__state.conversation) return false

  let cand: CandidateOutput | undefined
  try {
    cand = (await ApiClient.Workflow.readOutput({
      run_id: runId,
      step_id: candStep,
    })) as CandidateOutput
  } catch {
    return false
  }
  const records = cand?.records
  if (!cand || !Array.isArray(records)) return false

  // The AI first-pass decisions are OPTIONAL: if the `screen` step output is
  // unreadable, still open the panel with the candidate records (just no
  // pre-seeded include/exclude).
  let screenRaw: unknown = []
  try {
    screenRaw = await ApiClient.Workflow.readOutput({ run_id: runId, step_id: 'screen' })
  } catch {
    screenRaw = []
  }

  // Index the AI first-pass decisions by id (doi/pmid), then re-key by the
  // panel's record key so include/exclude line up with the rendered rows.
  const ai: AiDecision[] = Array.isArray(screenRaw)
    ? (screenRaw as AiDecision[]).filter(Boolean)
    : []
  const byId = new Map<string, AiDecision>()
  for (const d of ai) if (d?.id) byId.set(String(d.id).toLowerCase(), d)

  const decisions: Record<string, ScreeningDecision> = {}
  const reasons: Record<string, string> = {}
  for (const r of records) {
    const k = recordKey(r)
    const d =
      (r.doi && byId.get(r.doi.toLowerCase())) ||
      (r.pmid && byId.get(String(r.pmid).toLowerCase())) ||
      undefined
    if (d) {
      if (d.decision === 'include' || d.decision === 'exclude') decisions[k] = d.decision
      if (d.reason) reasons[k] = d.reason
    }
  }

  const query = cand.query || 'Systematic review'
  const sessionId = `wf-sr:${runId}`
  const data: LiteratureScreeningData = {
    sessionId,
    query,
    records,
    identified: cand.identified ?? {},
    afterDedup: cand.after_dedup ?? records.length,
    degradedSources: cand.degraded_sources ?? [],
    completeness: cand.completeness ?? null,
    decisions,
    reasons,
  }
  Stores.Chat.__state.displayInRightPanel({
    id: sessionId,
    title: `Screening: ${query}`.slice(0, 60),
    type: 'literature',
    data,
  })
  return true
}
