import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  ItemProgress,
  SSEElicitationRequiredData,
} from '@/api-client/types'
import {
  type RunProgressSubscription,
  subscribeRunProgress,
} from '@/modules/workflow/sse/runProgressClient'

/** Per-step UI state aggregated from the WorkflowRunEvent stream. */
export interface StepProgress {
  stepId: string
  stepKind?: string
  stepIndex?: number
  message?: string
  status: 'pending' | 'running' | 'completed' | 'failed'
  itemProgress?: ItemProgress
  outputPreview?: string
  error?: string
  tokensUsed?: number
  msElapsed?: number
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
                v.pendingElicitation = d.pending_elicitation_json ?? undefined
                draft.runs[runId] = v
              })
            },
            runStarted: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                v.totalSteps = d.total_steps
                v.status = 'running'
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
            stepCompleted: d => {
              set(draft => {
                const v = draft.runs[runId] ?? blankView(runId)
                const s = ensureStep(v, d.step_id)
                s.status = 'completed'
                s.outputPreview = d.output_preview
                s.tokensUsed = d.tokens_used
                s.msElapsed = d.ms_elapsed
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
