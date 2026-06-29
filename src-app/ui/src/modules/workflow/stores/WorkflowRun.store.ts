import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  ItemProgress,
  ProgressTrack,
  SSEElicitationRequiredData,
  SSEStepManifestItem,
} from '@/api-client/types'
import {
  type RunProgressSubscription,
  subscribeRunProgress,
} from '@/modules/workflow/sse/runProgressClient'

/** Per-step output metadata (mirrors backend `OutputMeta`). Content
 *  lives on disk; this is the snapshot blob carried in
 *  `step_outputs_json[step_id]`. Its presence means a full output file
 *  exists and can be fetched via `readOutput`. */
export interface StepOutputMeta {
  path?: string
  size_bytes?: number
  sha256?: string
  preview?: string
  kind?: string
  parsed_as?: 'json' | 'text'
}

/** Per-step artifact metadata (mirrors backend `ArtifactMeta`). One
 *  entry per file in `artifacts/<step_id>/`; fetched via
 *  `readArtifact`. */
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
  /** Live sandbox-step progress tracks (P2), keyed by track id ("" = default).
   *  Hydrated from the snapshot's `step_progress_json` + `stepProgress` deltas. */
  tracks?: Record<string, ProgressTrack>
  outputPreview?: string
  error?: string
  tokensUsed?: number
  msElapsed?: number
  // True once a completed step has a full output file on disk (set from
  // the snapshot's step_outputs_json or on stepCompleted). Drives the
  // "Show full output" expander.
  hasOutput?: boolean
  outputMeta?: StepOutputMeta
  // Files the step wrote to artifacts/<step_id>/ (from the snapshot's
  // step_artifacts_json). Rendered as attachment-style blocks.
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

interface WorkflowRunState {
  runs: Record<string, RunView>
  cancelling: Record<string, boolean>
  submittingElicit: Record<string, boolean>

  /** Open (or re-open) the SSE stream for a run. Idempotent per run. */
  subscribe: (runId: string) => void
  /** Tear down the SSE stream for a run. */
  unsubscribe: (runId: string) => void
  cancel: (runId: string) => Promise<void>
  submitElicitation: (
    runId: string,
    elicitationId: string,
    response: any,
  ) => Promise<void>
}

// SSE subscriptions live outside immer state (AbortControllers aren't
// draftable). Keyed by run id, doubling as a double-subscribe guard.
const subscriptions: Record<string, RunProgressSubscription> = {}

function ensureStep(view: RunView, stepId: string): StepProgress {
  if (!view.steps[stepId]) {
    view.steps[stepId] = { stepId, status: 'pending' }
    view.stepOrder.push(stepId)
  }
  return view.steps[stepId]
}

/** Seed the full pipeline from the SSE manifest (P1 Option B) so a (re)connect
 *  renders every step up front — pending ones included — in topo order. Fills
 *  kind + description without overwriting values a live event already set. */
function seedManifest(view: RunView, manifest?: SSEStepManifestItem[] | null) {
  if (!manifest) return
  for (const item of manifest) {
    const s = ensureStep(view, item.id)
    if (s.stepKind === undefined) s.stepKind = item.kind
    if (s.description === undefined && item.description != null) {
      s.description = item.description
    }
  }
}

function blankView(runId: string): RunView {
  return {
    runId,
    status: 'pending',
    totalTokens: 0,
    steps: {},
    stepOrder: [],
    connected: false,
  }
}

export const useWorkflowRunStore = create<WorkflowRunState>()(
  subscribeWithSelector(
    immer(
      (set, get): WorkflowRunState => ({
        runs: {},
        cancelling: {},
        submittingElicit: {},

        subscribe: (runId: string) => {
          if (subscriptions[runId]) return
          set(draft => {
            if (!draft.runs[runId]) draft.runs[runId] = blankView(runId)
          })
          subscriptions[runId] = subscribeRunProgress(runId, {
            connected: () => {
              set(draft => {
                const v = draft.runs[runId]
                if (v) v.connected = true
              })
            },
            snapshot: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                v.status = d.status
                v.totalTokens = d.total_tokens
                v.currentStep = d.current_step ?? undefined
                v.pendingElicitation =
                  (d.pending_elicitation_json as
                    | SSEElicitationRequiredData
                    | undefined) ?? undefined
                // P1: seed the full pipeline (pending steps incl.) up front.
                seedManifest(v, d.step_manifest)
                // P2: rehydrate the running step's in-flight tracks after a
                // refresh/reconnect (they belong to `current_step`).
                if (d.step_progress_json && d.current_step) {
                  const s = ensureStep(v, d.current_step)
                  s.tracks = d.step_progress_json as Record<string, ProgressTrack>
                }
                // Hydrate per-step output + artifact metadata so a
                // freshly-mounted view (or a reconnect) renders the
                // "Show full output" expander + artifact blocks without
                // a separate GET /workflow-runs/{id} call. The blobs are
                // metadata only (path/size/preview/mime); content is
                // fetched lazily via readOutput / readArtifact.
                const outputs = (d.step_outputs_json ?? {}) as Record<
                  string,
                  StepOutputMeta
                >
                for (const [stepId, meta] of Object.entries(outputs)) {
                  const s = ensureStep(v, stepId)
                  s.outputMeta = meta
                  s.hasOutput = true
                  if (!s.outputPreview && meta?.preview) {
                    s.outputPreview = meta.preview
                  }
                  if (s.status === 'pending') s.status = 'completed'
                }
                const artifacts = (d.step_artifacts_json ?? {}) as Record<
                  string,
                  StepArtifactMeta[]
                >
                for (const [stepId, list] of Object.entries(artifacts)) {
                  if (!Array.isArray(list) || list.length === 0) continue
                  const s = ensureStep(v, stepId)
                  s.artifacts = list
                }
                draft.runs[runId] = v
              })
            },
            runStarted: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                v.totalSteps = d.total_steps
                v.status = 'running'
                // P1: live first-paint of the full pipeline.
                seedManifest(v, d.step_manifest)
                draft.runs[runId] = v
              })
            },
            stepStarted: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                const s = ensureStep(v, d.step_id)
                s.status = 'running'
                s.stepKind = d.step_kind
                s.stepIndex = d.step_index
                s.message = d.message ?? undefined
                // P1: upgrade the label to the full-context render (keep the
                // inputs-rendered manifest value if this step omits one).
                if (d.description != null) s.description = d.description
                v.totalSteps = d.total_steps
                v.currentStep = d.step_id
                draft.runs[runId] = v
              })
            },
            stepItemProgress: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                const s = ensureStep(v, d.step_id)
                s.itemProgress = d.progress
                draft.runs[runId] = v
              })
            },
            stepProgress: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                const s = ensureStep(v, d.step_id)
                if (!s.tracks) s.tracks = {}
                for (const t of d.tracks) {
                  const id = t.id ?? ''
                  // `done` tracks were delivered once + evicted backend-side;
                  // drop them here too so live + refresh stay consistent.
                  if (t.done) delete s.tracks[id]
                  else s.tracks[id] = t
                }
                draft.runs[runId] = v
              })
            },
            stepCompleted: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                const s = ensureStep(v, d.step_id)
                s.status = 'completed'
                s.outputPreview = d.output_preview
                s.tokensUsed = d.tokens_used
                s.msElapsed = d.ms_elapsed
                // A completed step has a full output file on disk; the
                // SSE frame only carries a 500-char preview. Flag so the
                // "Show full output" expander mounts (it fetches the
                // full bytes via readOutput). Artifact metadata arrives
                // on the next snapshot (reconnect / mount).
                s.hasOutput = true
                v.totalTokens += d.tokens_used
                draft.runs[runId] = v
              })
            },
            stepFailed: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                const s = ensureStep(v, d.step_id)
                s.status = 'failed'
                s.error = d.error
                s.tokensUsed = d.tokens_used
                draft.runs[runId] = v
              })
            },
            elicitationRequired: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                v.pendingElicitation = d
                draft.runs[runId] = v
              })
            },
            elicitationResolved: () => {
              set(draft => {
                const v = draft.runs[runId]
                if (v) v.pendingElicitation = undefined
              })
            },
            runCompleted: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                v.status = 'completed'
                v.totalTokens = d.total_tokens
                draft.runs[runId] = v
              })
              get().unsubscribe(runId)
            },
            runCancelled: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                v.status = 'cancelled'
                v.totalTokens = d.total_tokens
                draft.runs[runId] = v
              })
              get().unsubscribe(runId)
            },
            runFailed: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                v.status = 'failed'
                v.error = d.error
                v.totalTokens = d.total_tokens
                draft.runs[runId] = v
              })
              get().unsubscribe(runId)
            },
            disconnected: () => {
              set(draft => {
                const v = draft.runs[runId]
                if (v) v.connected = false
              })
            },
          })
        },

        unsubscribe: (runId: string) => {
          subscriptions[runId]?.close()
          delete subscriptions[runId]
        },

        cancel: async (runId: string) => {
          set(draft => {
            draft.cancelling[runId] = true
          })
          try {
            await ApiClient.Workflow.cancelRun({ run_id: runId })
          } catch (e) {
            // M-7: callers fire-and-forget via `void`, so swallow-and-surface
            // here rather than reject unhandled. Show the failure in the run's
            // error banner.
            set(draft => {
              const v = draft.runs[runId]
              if (v) v.error = `Failed to cancel run: ${String(e)}`
            })
          } finally {
            set(draft => {
              delete draft.cancelling[runId]
            })
          }
        },

        submitElicitation: async (
          runId: string,
          elicitationId: string,
          response: any,
        ) => {
          set(draft => {
            draft.submittingElicit[runId] = true
          })
          try {
            await ApiClient.Workflow.submitElicit({
              run_id: runId,
              elicitation_id: elicitationId,
              response,
            })
            set(draft => {
              const v = draft.runs[runId]
              if (v) v.pendingElicitation = undefined
            })
          } catch (e) {
            // M-7: surface a failed submission instead of rejecting unhandled.
            set(draft => {
              const v = draft.runs[runId]
              if (v) v.error = `Failed to submit response: ${String(e)}`
            })
          } finally {
            set(draft => {
              delete draft.submittingElicit[runId]
            })
          }
        },
      }),
    ),
  ),
)
