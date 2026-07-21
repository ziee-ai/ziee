import type { StoreSet } from '@ziee/framework/store-kit'
import type {
  ItemProgress,
  ProgressTrack,
  SSEElicitationRequiredData,
  SSEStepManifestItem,
} from '@/api-client/types'

/** Per-step output metadata (mirrors backend `OutputMeta`). Content lives on
 *  disk; this is the snapshot blob in `step_outputs_json[step_id]`. Its
 *  presence means a full output file exists and can be fetched via `readOutput`. */
export interface StepOutputMeta {
  path?: string
  size_bytes?: number
  sha256?: string
  preview?: string
  kind?: string
  parsed_as?: 'json' | 'text'
}

/** Per-step artifact metadata (mirrors backend `ArtifactMeta`). */
export interface StepArtifactMeta {
  filename: string
  size_bytes?: number
  sha256?: string
  mime_type?: string
  description?: string
}

/** Per-step UI state aggregated from the WorkflowRunEvent stream. */
export interface StepProgress {
  stepId: string
  stepKind?: string
  stepIndex?: number
  message?: string
  /** Author-facing step label (P1). Manifest seeds the inputs-rendered value;
   *  `stepStarted` upgrades it to the full-context render. */
  description?: string
  status: 'pending' | 'running' | 'completed' | 'failed'
  itemProgress?: ItemProgress
  /** Live sandbox-step progress tracks (P2), keyed by track id ("" = default). */
  tracks?: Record<string, ProgressTrack>
  outputPreview?: string
  error?: string
  tokensUsed?: number
  msElapsed?: number
  hasOutput?: boolean
  outputMeta?: StepOutputMeta
  artifacts?: StepArtifactMeta[]
}

/** Aggregated live view of a single run, keyed by run id. */
export interface RunView {
  runId: string
  status: string
  totalSteps?: number
  totalTokens: number
  currentStep?: string
  steps: Record<string, StepProgress>
  stepOrder: string[]
  pendingElicitation?: SSEElicitationRequiredData
  error?: string
  connected: boolean
}

// SSE subscriptions live outside immer state (AbortControllers aren't
// draftable). Keyed by run id, doubling as a double-subscribe guard.
export const subscriptions: Record<string, any> = {}

export function ensureStep(view: RunView, stepId: string): StepProgress {
  if (!view.steps[stepId]) {
    view.steps[stepId] = { stepId, status: 'pending' }
    view.stepOrder.push(stepId)
  }
  return view.steps[stepId]
}

/** Seed the full pipeline from the SSE manifest (P1 Option B) so a (re)connect
 *  renders every step up front — pending included — in topo order. */
export function seedManifest(view: RunView, manifest?: SSEStepManifestItem[] | null) {
  if (!manifest) return
  for (const item of manifest) {
    const s = ensureStep(view, item.id)
    if (s.stepKind === undefined) s.stepKind = item.kind
    if (s.description === undefined && item.description != null) s.description = item.description
  }
}

export function blankView(runId: string): RunView {
  return { runId, status: 'pending', totalTokens: 0, steps: {}, stepOrder: [], connected: false }
}

export const workflowRunState = {
  runs: {} as Record<string, RunView>,
  cancelling: {} as Record<string, boolean>,
  submittingElicit: {} as Record<string, boolean>,
}

export type WorkflowRunState = typeof workflowRunState
export type WorkflowRunSet = StoreSet<WorkflowRunState>
export type WorkflowRunGet = () => WorkflowRunState
