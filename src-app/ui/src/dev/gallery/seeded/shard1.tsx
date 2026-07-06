/**
 * Shard 1 seeded-surface entries (parallel gap grind) — Workflow module.
 *
 * OWNED BY SHARD 1 ONLY. Every slug is prefixed `seeded-s1-`. Do NOT edit
 * seededSurfaces.tsx, overlays.tsx, main.tsx, pages.tsx, stories/index.ts,
 * coverage-allowlist.json, or any generated matrix — those are integrator-owned.
 *
 * See /data/pbya/ziee/tmp/gapgrind-shards.md for the assigned gap list.
 */
import { lazy, useEffect, useRef } from 'react'
import type {
  DryRunResult,
  SSEElicitationRequiredData,
  TestRunResponse,
  Workflow,
} from '@/api-client/types'
import { type SeededSurfaceEntry, holdPatch, lazyProps } from './helpers'

/** Minimal Workflow object — the dialogs only read `.id`. */
const galleryWorkflow = {
  id: 'wf-s1',
  name: 'Gallery workflow',
} as unknown as Workflow

/** A canned test run with a passed, a failed (with failure detail), and a
 *  skipped fixture so the passed/failed/skipped tags + the failure branch all
 *  render (WorkflowTestsPanel :62,66,67 + the `r.failure` arm). */
const cannedTestResult: TestRunResponse = {
  total: 3,
  passed: 1,
  failed: 1,
  skipped: 1,
  results: [
    { name: 'greets the user', passed: true, duration_ms: 42 },
    {
      name: 'summarizes the abstract',
      passed: false,
      duration_ms: 118,
      failure: {
        output_name: 'summary',
        assertion: 'contains',
        expected: '"insulin resistance"',
        actual_preview: '"the study examined blood glucose…"',
      },
    },
    { name: 'real_llm end-to-end', passed: false, skipped: true, duration_ms: 0 },
  ],
}

/** A canned dry-run with a runtime-dependent step so the estimate table, the
 *  cost statistic, and the `runtime-dependent` cell all render. */
const cannedDryRunResult: DryRunResult = {
  total_est_calls: 7,
  total_est_tokens: 12800,
  est_cost_usd: 0.0384,
  steps: [
    {
      step_id: 'draft',
      kind: 'llm',
      est_calls: 1,
      est_tokens_in: 900,
      est_tokens_out: 1200,
      runtime_dependent: false,
    },
    {
      step_id: 'map_sections',
      kind: 'llm_map',
      est_calls: 6,
      est_tokens_in: 5400,
      est_tokens_out: 5300,
      runtime_dependent: true,
    },
  ],
}

/** An elicitation whose schema has a required string field left empty, so a
 *  programmatic submit-click fails validation and lights the inline error. */
const galleryElicitation: SSEElicitationRequiredData = {
  elicitation_id: 'elicit-s1',
  run_id: 'run-s1',
  step_id: 'ask',
  message: 'Please provide the missing details before continuing.',
  deadline_at: new Date(Date.now() + 600_000).toISOString(),
  schema: {
    type: 'object',
    properties: {
      title: { type: 'string', title: 'Manuscript title' },
    },
    required: ['title'],
  },
}

/**
 * Build a lazy wrapper for the modal panels (tests / dry-run) whose loading /
 * error / result branches live in LOCAL `useState`, driven entirely by the
 * outcome of a `Stores.Workflow` action. We patch that action on the store —
 * inside the async lazy loader, BEFORE the panel ever mounts — so the panel's
 * mount-effect call resolves/rejects/hangs into the branch we want.
 */
function panelSurface(
  loader: () => Promise<Record<string, any>>,
  panelName: 'WorkflowTestsPanel' | 'DryRunPreviewDialog',
  actionName: 'test' | 'dryRun',
  outcome: 'loading' | 'error' | 'result',
  result: unknown,
) {
  return lazy(async () => {
    const store = await import('@/modules/workflow/stores/Workflow.store')
    const mod = await loader()
    const Panel = mod[panelName]
    const impl =
      outcome === 'loading'
        ? () => new Promise<never>(() => {})
        : outcome === 'error'
          ? () =>
              Promise.reject(
                new Error('Upstream returned 500 — service unavailable'),
              )
          : () => Promise.resolve(result)
    store.WorkflowStoreDef.store.setState({ [actionName]: impl } as any)
    return {
      default: () => (
        <Panel workflow={galleryWorkflow} open onClose={() => undefined} />
      ),
    }
  })
}

/** WorkflowElicitForm wrapper that auto-clicks Submit (with a required field
 *  left blank) so validation fails and the inline error alert renders
 *  (:484,485). */
const elicitErrorSurface = lazy(async () => {
  const mod = await import('@/modules/workflow/components/WorkflowElicitForm')
  const WorkflowElicitForm = mod.WorkflowElicitForm
  return {
    default: () => {
      const ref = useRef<HTMLDivElement>(null)
      useEffect(() => {
        // The form finishes async validation after the first mount tick;
        // retrying the click keeps the error branch lit for the render pass.
        let n = 0
        const t = setInterval(() => {
          // Select the Submit button by role, not its testid literal (the
          // testid-unique guard forbids repeating a data-testid string here).
          const btn = ref.current?.querySelector<HTMLButtonElement>(
            'button[type="button"]',
          )
          btn?.click()
          if (++n > 12) clearInterval(t)
        }, 200)
        return () => clearInterval(t)
      }, [])
      return (
        <div ref={ref}>
          <WorkflowElicitForm
            elicitation={galleryElicitation}
            submitting={false}
            onSubmit={() => undefined}
          />
        </div>
      )
    },
  }
})

/** EditableArrayTable inside a RHF Form whose field array is empty → the
 *  "No rows" empty `<tr>` (:358). */
const arrayEmptySurface = lazy(async () => {
  const { Form, useForm } = await import('@/components/ui')
  const { EditableArrayTable } = await import(
    '@/modules/workflow/components/EditableArrayTable'
  )
  const arraySchema = {
    type: 'array',
    title: 'Rows',
    items: {
      type: 'object',
      properties: { label: { type: 'string', title: 'Label' } },
      required: [],
    },
  } as any
  return {
    default: () => {
      const form = useForm({ defaultValues: { rows: [] } })
      return (
        <div className="p-4">
          <Form
            data-testid="s1-array-empty-form"
            form={form}
            onSubmit={() => undefined}
          >
            <EditableArrayTable name="rows" schema={arraySchema} />
          </Form>
        </div>
      )
    },
  }
})

export const shard1Seeded: SeededSurfaceEntry[] = [
  // ── WorkflowRunProgressView: a failed run with a completed + a failed step →
  //    run-error alert (:178), per-step error (:240,241), and the
  //    completed/failed log-expander block (:268,269). ─────────────────────────
  {
    slug: 'seeded-s1-run-progress-error',
    title: 'Workflow run progress — failed run',
    note: 'run.error + a failed step (error) + completed step → error alert, step error, log expanders',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/workflow/components/WorkflowRunProgressView'),
      'WorkflowRunProgressView',
      { runId: 'run-s1-err' },
    ),
    setup: async () => {
      const { WorkflowRun } = await import(
        '@/modules/workflow/stores/WorkflowRun.store'
      )
      await holdPatch(() =>
        WorkflowRun.store.setState({
          runs: {
            'run-s1-err': {
              runId: 'run-s1-err',
              status: 'failed',
              totalTokens: 1840,
              connected: true,
              error: 'Run failed: sandbox step exited non-zero',
              stepOrder: ['draft', 'analyze'],
              steps: {
                draft: {
                  stepId: 'draft',
                  stepKind: 'llm',
                  status: 'completed',
                  description: 'Draft the outline',
                  tokensUsed: 512,
                  msElapsed: 2300,
                  hasOutput: true,
                },
                analyze: {
                  stepId: 'analyze',
                  stepKind: 'sandbox',
                  status: 'failed',
                  description: 'Run the analysis script',
                  error: 'Command exited with code 1: ModuleNotFoundError',
                },
              },
            },
          },
        } as any),
      )
    },
  },
  // ── WorkflowRunProgressView: a non-terminal run with no steps yet → the
  //    "Waiting for steps to start…" empty arm (:307). ─────────────────────────
  {
    slug: 'seeded-s1-run-progress-empty-steps',
    title: 'Workflow run progress — awaiting steps',
    note: 'non-terminal run with steps:{} stepOrder:[] → "Waiting for steps to start…"',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/workflow/components/WorkflowRunProgressView'),
      'WorkflowRunProgressView',
      { runId: 'run-s1-empty' },
    ),
    setup: async () => {
      const { WorkflowRun } = await import(
        '@/modules/workflow/stores/WorkflowRun.store'
      )
      await holdPatch(() =>
        WorkflowRun.store.setState({
          runs: {
            'run-s1-empty': {
              runId: 'run-s1-empty',
              status: 'running',
              totalTokens: 0,
              connected: true,
              stepOrder: [],
              steps: {},
            },
          },
        } as any),
      )
    },
  },
  // ── WorkflowTestsPanel: loading / error / result (local useState driven by
  //    Stores.Workflow.test). :60 / :61 / :62,66,67. ──────────────────────────
  {
    slug: 'seeded-s1-tests-loading',
    title: 'Workflow tests — loading',
    note: 'test() pending → the load spinner',
    path: '/',
    initialPath: '/',
    component: panelSurface(() => import('@/modules/workflow/components/WorkflowTestsPanel'), 'WorkflowTestsPanel', 'test', 'loading', null),
  },
  {
    slug: 'seeded-s1-tests-error',
    title: 'Workflow tests — error',
    note: 'test() rejects → the error alert',
    path: '/',
    initialPath: '/',
    component: panelSurface(() => import('@/modules/workflow/components/WorkflowTestsPanel'), 'WorkflowTestsPanel', 'test', 'error', null),
  },
  {
    slug: 'seeded-s1-tests-result',
    title: 'Workflow tests — results',
    note: 'test() resolves with passed/failed/skipped → tags, list, failure detail',
    path: '/',
    initialPath: '/',
    component: panelSurface(
      () => import('@/modules/workflow/components/WorkflowTestsPanel'),
      'WorkflowTestsPanel',
      'test',
      'result',
      cannedTestResult,
    ),
  },
  // ── DryRunPreviewDialog: loading / error / result (Stores.Workflow.dryRun).
  //    :58 / :59 / :60. ───────────────────────────────────────────────────────
  {
    slug: 'seeded-s1-dry-run-loading',
    title: 'Workflow dry-run — loading',
    note: 'dryRun() pending → the "Running dry run" spinner',
    path: '/',
    initialPath: '/',
    component: panelSurface(() => import('@/modules/workflow/components/DryRunPreviewDialog'), 'DryRunPreviewDialog', 'dryRun', 'loading', null),
  },
  {
    slug: 'seeded-s1-dry-run-error',
    title: 'Workflow dry-run — error',
    note: 'dryRun() rejects → the error alert',
    path: '/',
    initialPath: '/',
    component: panelSurface(() => import('@/modules/workflow/components/DryRunPreviewDialog'), 'DryRunPreviewDialog', 'dryRun', 'error', null),
  },
  {
    slug: 'seeded-s1-dry-run-result',
    title: 'Workflow dry-run — results',
    note: 'dryRun() resolves → est stats + per-step table (runtime-dependent cell)',
    path: '/',
    initialPath: '/',
    component: panelSurface(
      () => import('@/modules/workflow/components/DryRunPreviewDialog'),
      'DryRunPreviewDialog',
      'dryRun',
      'result',
      cannedDryRunResult,
    ),
  },
  // ── WorkflowElicitForm: validation-failed submit → the inline error alert
  //    (:484,485). Auto-clicked after mount with a required field left blank. ──
  {
    slug: 'seeded-s1-elicit-error',
    title: 'Workflow elicitation — validation error',
    note: 'required field blank + submit → "Please fix the highlighted fields" alert',
    path: '/',
    initialPath: '/',
    component: elicitErrorSurface,
  },
  // ── EditableArrayTable: an empty RHF field array → the "No rows" empty
  //    <tr> (:358). ───────────────────────────────────────────────────────────
  {
    slug: 'seeded-s1-array-empty',
    title: 'Workflow editable array table — empty',
    note: 'empty field array → the "No rows" empty row',
    path: '/',
    initialPath: '/',
    component: arrayEmptySurface,
  },
]
