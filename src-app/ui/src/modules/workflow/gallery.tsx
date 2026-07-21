/**
 * Dev-gallery seed for the `workflow` module — the workflow cassette, overlay
 * open-states (assignment / detail / import / run / dry-run / tests), and seeded
 * surfaces (run-progress, tests, dry-run, elicitation, editable-array, runs list,
 * step artifacts). Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import { lazy, useEffect, useRef } from 'react'
import type {
  DryRunResult,
  SSEElicitationRequiredData,
  TestRunResponse,
  ValidateDefResponse,
  Workflow,
} from '@/api-client/types'
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyBound, lazyNamed, lazyProps } from '@/dev/gallery/support'
import { Stores } from '@ziee/framework/stores'
import type { BuilderDef } from './stores/WorkflowBuilder.store'
import { workflowCassette } from '@/dev/gallery/fixtures/workflow'
import { llmGroupsList } from '@/dev/gallery/fixtures/llm-providers'

const group = llmGroupsList.groups[0]

const noop = () => {}

const workflowFixture = {
  id: 'wf-gallery-0001',
  name: 'Weekly literature digest',
  description: 'Search, screen, and summarize new papers on a saved query.',
  scope: 'user',
  version: '1.0.0',
  is_system: false,
  enabled: true,
  created_at: '2026-02-01T10:00:00Z',
  updated_at: '2026-02-01T10:00:00Z',
  compiled_ir_json: {
    inputs: [
      { name: 'query', description: 'Search terms', required: true },
      { name: 'max_results', description: 'Cap', required: false, default: 20 },
    ],
    steps: [{ id: 'search' }, { id: 'summarize' }],
  },
} as const

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
  const { Form, useForm } = await import('@ziee/kit')
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

// ── Builder + agent-timeline fixtures (ITEM-7/9) ────────────────────────────

/** A representative, fully-populated agent step: a plain-language task, two
 *  selected capabilities, Balanced effort (max_steps 30), Text output, and a
 *  system directive — the centrepiece the friendly agent form renders. */
const agentStepFixture = {
  id: 'agent_1',
  kind: 'agent',
  description: 'Research the topic',
  depends_on: [],
  prompt:
    'Find the three most-cited papers on CRISPR base editing published since 2023 and summarise their key findings in plain language for a non-specialist.',
  system: 'You are a meticulous research assistant. Cite every claim with a DOI.',
  servers: ['literature_search', 'web_search'],
  max_steps: 30,
  output_format: 'text',
}

/** A representative 4-step workflow (agent → llm → elicit → sandbox) so the
 *  populated builder shows real-data master/detail layout. */
const builderFourStepDef = {
  inputs: [
    { name: 'topic', description: 'The research topic', required: true },
    {
      name: 'since_year',
      description: 'Earliest publication year',
      required: false,
      default: 2023,
    },
  ],
  steps: [
    agentStepFixture,
    {
      id: 'summarize',
      kind: 'llm',
      description: 'Summarise the findings',
      depends_on: ['agent_1'],
      prompt: 'Summarise the key points from {{ agent_1.output }} in five bullets.',
      output_format: 'text',
      tools: [],
    },
    {
      id: 'review',
      kind: 'elicit',
      description: 'Human review',
      depends_on: ['summarize'],
      message: 'Does this summary look right before we export it?',
      schema: {
        type: 'object',
        properties: { approved: { type: 'boolean', title: 'Approved' } },
      },
      timeout_ms: 300_000,
    },
    {
      id: 'export',
      kind: 'sandbox',
      description: 'Export to PDF',
      depends_on: ['review'],
      run: 'pandoc summary.md -o digest.pdf',
      stdin: null,
      timeout_ms: 30_000,
    },
  ],
} as unknown as BuilderDef

/** A clean validation (no errors) with a cost estimate — the populated-builder
 *  happy path. */
const cleanValidation: ValidateDefResponse = {
  errors: [],
  warnings: [],
  cost_estimate: cannedDryRunResult,
}

/** A validation with ≥1 error + ≥1 warning + a cost estimate — the panel's
 *  error/warning/cost branches all lit. */
const errorValidation: ValidateDefResponse = {
  errors: [
    {
      code: 'unresolved_reference',
      layer: 'graph',
      location: 'summarize',
      message:
        'Step "Summarise the findings" references {{ agent_1.output }}, but the agent step produces no named output — give the agent a Structured output or reference its text result.',
    },
  ],
  warnings: [
    {
      code: 'long_prompt',
      layer: 'lint',
      location: 'agent_1',
      message:
        'This task prompt is long; consider splitting it into two steps for clearer run progress.',
    },
  ],
  cost_estimate: cannedDryRunResult,
}

/** The friendly agent form (ITEM-9), populated — a wrapper instantiates the
 *  per-instance builder store with the agent step seeded as its initial state
 *  (no network), then renders the real form with a store + step prop. */
const agentFormSurface = lazy(async () => {
  const { WorkflowBuilderStoreDef } = await import(
    '@/modules/workflow/stores/WorkflowBuilder.store'
  )
  const { AgentStepForm } = await import(
    '@/modules/workflow/components/builder/AgentStepForm'
  )
  return {
    default: () => {
      const store = WorkflowBuilderStoreDef.use({
        def: { inputs: [], steps: [agentStepFixture] } as unknown as BuilderDef,
        selectedStepId: 'agent_1',
      })
      return (
        <div className="max-w-xl p-4">
          <AgentStepForm store={store} step={agentStepFixture as any} />
        </div>
      )
    },
  }
})

/** The populated builder (ITEM-7): step-list master + config-panel detail +
 *  inputs editor + validation panel, driven by a store seeded with the 4-step
 *  def and the agent step selected — so the detail column shows the agent form. */
const populatedBuilderSurface = lazy(async () => {
  const { WorkflowBuilderStoreDef } = await import(
    '@/modules/workflow/stores/WorkflowBuilder.store'
  )
  const { StepList } = await import(
    '@/modules/workflow/components/builder/StepList'
  )
  const { StepConfigPanel } = await import(
    '@/modules/workflow/components/builder/StepConfigPanel'
  )
  const { WorkflowInputsEditor } = await import(
    '@/modules/workflow/components/builder/WorkflowInputsEditor'
  )
  const { BuilderValidationPanel } = await import(
    '@/modules/workflow/components/builder/BuilderValidationPanel'
  )
  return {
    default: () => {
      const store = WorkflowBuilderStoreDef.use({
        name: 'CRISPR literature digest',
        def: builderFourStepDef,
        selectedStepId: 'agent_1',
        validation: cleanValidation,
      })
      return (
        <div className="flex flex-col gap-4 p-4">
          <WorkflowInputsEditor store={store} />
          <div className="flex flex-col md:flex-row gap-4">
            <div className="md:w-80 shrink-0">
              <StepList store={store} />
            </div>
            <div className="flex-1 min-w-0">
              <StepConfigPanel store={store} />
            </div>
          </div>
          <BuilderValidationPanel store={store} />
        </div>
      )
    },
  }
})

/** The builder validation panel with ≥1 error + ≥1 warning + a cost estimate. */
const validationErrorSurface = lazy(async () => {
  const { WorkflowBuilderStoreDef } = await import(
    '@/modules/workflow/stores/WorkflowBuilder.store'
  )
  const { BuilderValidationPanel } = await import(
    '@/modules/workflow/components/builder/BuilderValidationPanel'
  )
  return {
    default: () => {
      const store = WorkflowBuilderStoreDef.use({ validation: errorValidation })
      return (
        <div className="max-w-2xl p-4">
          <BuilderValidationPanel store={store} />
        </div>
      )
    },
  }
})

export const gallery: ModuleGallery = {
  cassette: workflowCassette,
  overlays: [
    {
      slug: 'overlay-group-workflows-assignment',
      surface: 'modules/workflow/widgets/GroupSystemWorkflowsAssignmentDrawer',
      title: 'Group → Workflows (drawer)',
      component: lazyNamed(
        () => import('@/modules/workflow/widgets/GroupSystemWorkflowsAssignmentDrawer'),
        'GroupSystemWorkflowsAssignmentDrawer',
      ),
      open: () => Stores.GroupSystemWorkflowsAssignment.openDrawer(group),
    },
    {
      slug: 'overlay-workflow-detail-drawer',
      surface: 'modules/workflow/components/WorkflowDetailDrawer',
      title: 'Workflow detail (drawer)',
      // Wrapped in a MemoryRouter (dev-only) because the drawer's "Edit"
      // affordance calls useNavigate, which throws in the router-less gallery
      // overlay host. The real app always renders it inside the app Router.
      component: lazyNamed(
        () => import('@/dev/gallery/fixtures/routedOverlays'),
        'WorkflowDetailDrawerRouted',
      ),
      open: () => Stores.WorkflowDrawer.open(workflowFixture as any),
    },
    {
      slug: 'overlay-import-workflow-dialog',
      surface: 'modules/workflow/components/ImportWorkflowDialog',
      title: 'Import workflow (dialog)',
      component: lazyBound(
        () => import('@/modules/workflow/components/ImportWorkflowDialog'),
        'ImportWorkflowDialog',
        { open: true, onClose: noop },
      ),
    },
    {
      slug: 'overlay-workflow-run-dialog',
      surface: 'modules/workflow/components/WorkflowRunDialog',
      title: 'Run workflow (dialog)',
      component: lazyBound(
        () => import('@/modules/workflow/components/WorkflowRunDialog'),
        'WorkflowRunDialog',
        {
          open: true,
          onClose: noop,
          conversationId: 'conv-1',
          workflow: workflowFixture,
          onStarted: noop,
        },
      ),
    },
    {
      slug: 'overlay-dry-run-preview-dialog',
      surface: 'modules/workflow/components/DryRunPreviewDialog',
      title: 'Dry-run preview (dialog)',
      component: lazyBound(
        () => import('@/modules/workflow/components/DryRunPreviewDialog'),
        'DryRunPreviewDialog',
        { open: true, onClose: noop, workflow: workflowFixture },
      ),
    },
    {
      slug: 'overlay-workflow-tests-panel',
      surface: 'modules/workflow/components/WorkflowTestsPanel',
      title: 'Workflow tests (dialog)',
      component: lazyBound(
        () => import('@/modules/workflow/components/WorkflowTestsPanel'),
        'WorkflowTestsPanel',
        { open: true, onClose: noop, workflow: workflowFixture },
      ),
    },
  ],
  seeded: [
    // ── WorkflowRunsList: no runs for this workflow → empty (prop workflowId). ───
    {
      slug: 'seeded-workflow-runs-empty',
      title: 'Workflow runs list — empty',
      note: '!loading[wf] && items.length===0 → the empty state',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/workflow/components/WorkflowRunsList'),
        'WorkflowRunsList',
        { workflowId: 'wf-1', onSelectRun: () => undefined },
      ),
      setup: async () => {
        const { WorkflowRuns } = await import(
          '@/modules/workflow/stores/WorkflowRuns.store'
        )
        await holdPatch(() =>
          WorkflowRuns.store.setState({
            runs: { 'wf-1': [] },
            loading: { 'wf-1': false },
          } as any),
        )
      },
    },
    // ── StepArtifacts: a step with no artifacts → the `return null` arm. ─────────
    {
      slug: 'seeded-step-artifacts-empty',
      title: 'Workflow step artifacts — empty',
      note: 'artifacts.length===0 → renders nothing',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/workflow/components/StepArtifacts'),
        'StepArtifacts',
        { runId: 'run-1', stepId: 'step-1', artifacts: [] },
      ),
    },
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
    // ── Builder — friendly agent form (ITEM-9), populated. ──────────────────────
    {
      slug: 'seeded-wf-builder-agent-form',
      title: 'Workflow builder — agent task form',
      note: 'populated AgentStepForm: instructions + 2 capabilities + Balanced effort + Text output + read-back',
      path: '/',
      initialPath: '/',
      fullHeight: true,
      component: agentFormSurface,
    },
    // ── Builder — empty (create mode). Real WorkflowBuilderPage, no def. ────────
    {
      slug: 'seeded-wf-builder-empty',
      title: 'Workflow builder — new (empty)',
      note: 'WorkflowBuilderPage create mode (no :id) → initEmpty → empty step list + inputs',
      path: '/settings/workflows/builder',
      initialPath: '/settings/workflows/builder',
      fullHeight: true,
      component: lazyNamed(
        () =>
          import('@/modules/workflow/components/builder/WorkflowBuilderPage'),
        'WorkflowBuilderPage',
      ),
    },
    // ── Builder — populated (4-step workflow incl. an agent step). ─────────────
    {
      slug: 'seeded-wf-builder-populated',
      title: 'Workflow builder — populated (4 steps)',
      note: 'step-list + config-panel + inputs + validation, seeded with agent→llm→elicit→sandbox; agent step selected',
      path: '/',
      initialPath: '/',
      fullHeight: true,
      component: populatedBuilderSurface,
    },
    // ── Builder — validation panel with errors + warnings + cost. ──────────────
    {
      slug: 'seeded-wf-builder-validation-error',
      title: 'Workflow builder — validation errors',
      note: 'ValidateDefResponse with 1 error + 1 warning + a cost estimate → all panel branches',
      path: '/',
      initialPath: '/',
      fullHeight: true,
      component: validationErrorSurface,
    },
    // ── Run — agent activity timeline, RUNNING (last row status:running). ──────
    {
      slug: 'seeded-wf-run-agent-running',
      title: 'Agent run timeline — running',
      note: 'agent step with an accreting activity timeline; the last row is status:running',
      path: '/',
      initialPath: '/',
      fullHeight: true,
      component: lazyProps(
        () => import('@/modules/workflow/components/WorkflowRunProgressView'),
        'WorkflowRunProgressView',
        { runId: 'run-agent-running' },
      ),
      setup: async () => {
        const { WorkflowRun } = await import(
          '@/modules/workflow/stores/WorkflowRun.store'
        )
        await holdPatch(() =>
          WorkflowRun.store.setState({
            runs: {
              'run-agent-running': {
                runId: 'run-agent-running',
                status: 'running',
                totalTokens: 3420,
                connected: true,
                currentStep: 'agent_1',
                stepOrder: ['agent_1'],
                steps: {
                  agent_1: {
                    stepId: 'agent_1',
                    stepKind: 'agent',
                    status: 'running',
                    description: 'Research the topic',
                    agentActivity: [
                      { type: 'agent_activity', seq: 1, kind: 'tool_call', tool: 'literature_search', title: 'Searching the literature', detail: 'query: CRISPR base editing, 2023–2025', status: 'ok' },
                      { type: 'agent_activity', seq: 2, kind: 'tool_result', tool: 'literature_search', title: 'Found 24 papers', status: 'ok' },
                      { type: 'agent_activity', seq: 3, kind: 'tool_call', tool: 'fetch_paper_fulltext', title: 'Reading the 3 most-cited papers', status: 'ok' },
                      { type: 'agent_activity', seq: 4, kind: 'thinking', title: 'Comparing the key findings', status: 'ok' },
                      { type: 'agent_activity', seq: 5, kind: 'message', title: 'Drafting a plain-language summary', status: 'running' },
                    ],
                  },
                },
              },
            },
          } as any),
        )
      },
    },
    // ── Run — agent timeline with a PENDING GATE (inline elicitation form). ────
    {
      slug: 'seeded-wf-run-agent-gate',
      title: 'Agent run timeline — gate open',
      note: 'agent step paused on a human gate → inline WorkflowElicitForm anchored to the gate row',
      path: '/',
      initialPath: '/',
      fullHeight: true,
      component: lazyProps(
        () => import('@/modules/workflow/components/WorkflowRunProgressView'),
        'WorkflowRunProgressView',
        { runId: 'run-agent-gate' },
      ),
      setup: async () => {
        const { WorkflowRun } = await import(
          '@/modules/workflow/stores/WorkflowRun.store'
        )
        await holdPatch(() =>
          WorkflowRun.store.setState({
            runs: {
              'run-agent-gate': {
                runId: 'run-agent-gate',
                status: 'waiting',
                totalTokens: 5120,
                connected: true,
                currentStep: 'agent_1',
                pendingElicitation: {
                  elicitation_id: 'elicit-agent-1',
                  run_id: 'run-agent-gate',
                  step_id: 'agent_1',
                  message: 'I found two candidate review protocols. Which should I follow?',
                  deadline_at: new Date(Date.now() + 600_000).toISOString(),
                  schema: {
                    type: 'object',
                    properties: {
                      choice: { type: 'string', title: 'Which protocol should I use?' },
                    },
                    required: ['choice'],
                  },
                },
                stepOrder: ['agent_1'],
                steps: {
                  agent_1: {
                    stepId: 'agent_1',
                    stepKind: 'agent',
                    status: 'running',
                    description: 'Research the topic',
                    agentActivity: [
                      { type: 'agent_activity', seq: 1, kind: 'tool_call', tool: 'literature_search', title: 'Searching the literature', status: 'ok' },
                      { type: 'agent_activity', seq: 2, kind: 'tool_result', tool: 'literature_search', title: 'Found 24 papers', status: 'ok' },
                      { type: 'agent_activity', seq: 3, kind: 'gate', title: 'Waiting for your input', detail: 'The assistant paused to ask which review protocol to follow.', status: 'running' },
                    ],
                  },
                },
              },
            },
          } as any),
        )
      },
    },
    // ── Run — agent timeline, COMPLETED (all activity ok, step done). ──────────
    {
      slug: 'seeded-wf-run-agent-completed',
      title: 'Agent run timeline — completed',
      note: 'agent step with all activity status:ok and the step completed → output + log expanders',
      path: '/',
      initialPath: '/',
      fullHeight: true,
      component: lazyProps(
        () => import('@/modules/workflow/components/WorkflowRunProgressView'),
        'WorkflowRunProgressView',
        { runId: 'run-agent-done' },
      ),
      setup: async () => {
        const { WorkflowRun } = await import(
          '@/modules/workflow/stores/WorkflowRun.store'
        )
        await holdPatch(() =>
          WorkflowRun.store.setState({
            runs: {
              'run-agent-done': {
                runId: 'run-agent-done',
                status: 'completed',
                totalTokens: 6890,
                connected: true,
                stepOrder: ['agent_1'],
                steps: {
                  agent_1: {
                    stepId: 'agent_1',
                    stepKind: 'agent',
                    status: 'completed',
                    description: 'Research the topic',
                    tokensUsed: 6890,
                    msElapsed: 48200,
                    hasOutput: true,
                    outputPreview:
                      'Three papers stand out: Anzalone et al. (2023) on prime-editing efficiency, …',
                    agentActivity: [
                      { type: 'agent_activity', seq: 1, kind: 'tool_call', tool: 'literature_search', title: 'Searching the literature', status: 'ok' },
                      { type: 'agent_activity', seq: 2, kind: 'tool_result', tool: 'literature_search', title: 'Found 24 papers', status: 'ok' },
                      { type: 'agent_activity', seq: 3, kind: 'tool_call', tool: 'fetch_paper_fulltext', title: 'Read the 3 most-cited papers', status: 'ok' },
                      { type: 'agent_activity', seq: 4, kind: 'thinking', title: 'Compared the key findings', status: 'ok' },
                      { type: 'agent_activity', seq: 5, kind: 'message', title: 'Wrote the plain-language summary', status: 'ok' },
                    ],
                  },
                },
              },
            },
          } as any),
        )
      },
    },
  ],
}
