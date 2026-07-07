/**
 * Workflow fixture — seeds the two workflow dialogs that self-fetch on open:
 * `DryRunPreviewDialog` (POST /dry-run → DryRunResult) and `WorkflowTestsPanel`
 * (POST /test → TestRunResponse).
 *
 * Without these, both endpoints were UNRECORDED, so the mock returned the
 * `makeSafeEmpty()` proxy which serializes (via toJSON) to `[]`. The client then
 * saw `result = []` (a truthy array), `result.steps` / `result.results` resolved
 * to `undefined`, and the kit `Table` / `List` crashed on `dataSource.length` —
 * an AppErrorBoundary crash on both overlays (Round 3 runtime-health, opened).
 *
 * The kit primitives are now hardened (default `dataSource = []`), so a bad
 * shape no longer crashes; these fixtures additionally give each overlay a real
 * LOADED state (a varied step table + pass/fail/skip test list) so the audit
 * gets genuine signal instead of a degenerate empty render.
 *
 * Typed against the generated response types so a shape drift fails `tsc`; the
 * ajv contract test (`gallery:check-fixtures`) validates against openapi.json.
 */
import type { DryRunResult, TestRunResponse } from '@/api-client/types'
import type { Cassette } from '../mockApi'

const dryRun: DryRunResult = {
  total_est_calls: 7,
  total_est_tokens: 18_400,
  est_cost_usd: 0.0921,
  steps: [
    { step_id: 'search', kind: 'llm', est_calls: 1, est_tokens_in: 1200, est_tokens_out: 640, runtime_dependent: false },
    { step_id: 'summarize_each', kind: 'llm_map', est_calls: 5, est_tokens_in: 9800, est_tokens_out: 3200, runtime_dependent: true },
    { step_id: 'extract', kind: 'sandbox', est_calls: 0, est_tokens_in: 0, est_tokens_out: 0, runtime_dependent: false },
    { step_id: 'synthesize', kind: 'llm', est_calls: 1, est_tokens_in: 2600, est_tokens_out: 760, runtime_dependent: false },
  ],
}

const testRun: TestRunResponse = {
  total: 4,
  passed: 2,
  failed: 1,
  skipped: 1,
  results: [
    { name: 'search_returns_hits', passed: true, duration_ms: 412 },
    { name: 'summary_is_grounded', passed: true, duration_ms: 1893 },
    {
      name: 'synthesis_cites_sources',
      passed: false,
      duration_ms: 2044,
      failure: {
        output_name: 'citations',
        assertion: 'contains',
        expected: '≥ 3 references',
        actual_preview: 'found 1 reference (doi:10.48550/arXiv.1706.03762)',
      },
    },
    { name: 'real_llm_smoke', passed: false, skipped: true, duration_ms: 0 },
  ],
}

export const workflowCassette: Cassette = {
  'Workflow.dryRun': dryRun,
  'Workflow.test': testRun,
}
