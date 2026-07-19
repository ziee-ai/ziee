import { ApiClient } from '@/api-client'
import type {
  ItemProgress,
  ProgressTrack,
  SSEElicitationRequiredData,
  SSEStepManifestItem,
} from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'
import {
  type RunProgressSubscription,
  subscribeRunProgress,
} from '@/modules/workflow/sse/runProgressClient'
import type { AgentActivityEntry } from '@/modules/workflow/components/run/activityDescriptors'

/** Cap on the most-recent agent-activity rows retained per step in the client
 *  store. Mirrors the backend `AGENT_ACTIVITY_MAX_ENTRIES` (repository.rs) so a
 *  long run can't grow this array without bound. When exceeded we drop the
 *  lowest-`seq` (oldest) rows, matching the backend's chronological trim. */
const AGENT_ACTIVITY_MAX_ENTRIES = 500

/** Trim the (ascending-by-seq) list in place to the most-recent
 *  `AGENT_ACTIVITY_MAX_ENTRIES`, dropping the lowest-seq head. Bounded work
 *  (≤ overflow count), so it keeps per-frame merges O(1) amortized. */
function trimActivity(list: AgentActivityEntry[]) {
  if (list.length > AGENT_ACTIVITY_MAX_ENTRIES) {
    list.splice(0, list.length - AGENT_ACTIVITY_MAX_ENTRIES)
  }
}

/** Merge one agent-activity payload into an ordered, seq-deduped list (ascending
 *  by `seq`). Re-emitting the same seq (e.g. a `running`→`ok` status upgrade)
 *  REPLACES the existing row in place rather than appending a duplicate.
 *
 *  O(1) amortized: `seq` is monotonic and frames almost always arrive in order,
 *  so the common cases (new tail / tail status-upgrade) are constant time; only
 *  a genuinely out-of-order straggler pays an O(n) scan. The stored array is
 *  then capped so memory can't grow unbounded over a long run. */
function mergeAgentActivity(list: AgentActivityEntry[], entry: AgentActivityEntry) {
  const n = list.length
  if (n === 0) {
    list.push(entry)
  } else {
    const last = list[n - 1]
    if (entry.seq > last.seq) {
      // Common case: strictly newer → append (O(1)).
      list.push(entry)
    } else if (entry.seq === last.seq) {
      // Common case: status upgrade on the newest row → replace tail (O(1)).
      list[n - 1] = entry
    } else {
      // Rare: out-of-order seq. Locate the first row ≥ entry.seq and either
      // replace (dedupe) or splice-insert to preserve ascending order.
      const i = list.findIndex(e => e.seq >= entry.seq)
      if (i >= 0 && list[i].seq === entry.seq) list[i] = entry
      else if (i >= 0) list.splice(i, 0, entry)
      else list.push(entry)
    }
  }
  trimActivity(list)
}

/** Bulk-merge a persisted activity array into `list` in O(n + m) — one seq→index
 *  map, in-place replace for existing seqs, append + a single sort for the new
 *  ones — instead of an O(n²) per-element `findIndex`. Used by snapshot
 *  rehydrate, where the persisted array can be large. */
function mergeAgentActivityBatch(
  list: AgentActivityEntry[],
  incoming: AgentActivityEntry[],
) {
  if (incoming.length === 0) return
  const idxBySeq = new Map<number, number>()
  list.forEach((e, i) => idxBySeq.set(e.seq, i))
  let appended = false
  for (const e of incoming) {
    const i = idxBySeq.get(e.seq)
    if (i !== undefined) {
      list[i] = e // replace in place (dedupe / status upgrade)
    } else {
      list.push(e)
      appended = true
    }
  }
  // Persisted rows are chronological, but re-sort once only if we appended, to
  // restore the ascending-seq invariant defensively.
  if (appended) list.sort((a, b) => a.seq - b.seq)
  trimActivity(list)
}

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
  /** Ordered agent ACTIVITY TIMELINE for a `kind:agent` step — accreting rows
   *  (search/read/draft/gate…) deduped + ordered by `seq`. Fed by the
   *  `agent_activity` tracks on the StepProgress frame (kept OUT of `tracks`) and
   *  rehydrated from `step_logs_json["<stepId>::agent_activity"]` on snapshot. */
  agentActivity?: AgentActivityEntry[]
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
const subscriptions: Record<string, RunProgressSubscription> = {}

function ensureStep(view: RunView, stepId: string): StepProgress {
  if (!view.steps[stepId]) {
    view.steps[stepId] = { stepId, status: 'pending' }
    view.stepOrder.push(stepId)
  }
  return view.steps[stepId]
}

/** Seed the full pipeline from the SSE manifest (P1 Option B) so a (re)connect
 *  renders every step up front — pending included — in topo order. */
function seedManifest(view: RunView, manifest?: SSEStepManifestItem[] | null) {
  if (!manifest) return
  for (const item of manifest) {
    const s = ensureStep(view, item.id)
    if (s.stepKind === undefined) s.stepKind = item.kind
    if (s.description === undefined && item.description != null) s.description = item.description
  }
}

function blankView(runId: string): RunView {
  return { runId, status: 'pending', totalTokens: 0, steps: {}, stepOrder: [], connected: false }
}

export const WorkflowRun = defineStore('WorkflowRun', {
  immer: true,
  state: {
    runs: {} as Record<string, RunView>,
    cancelling: {} as Record<string, boolean>,
    submittingElicit: {} as Record<string, boolean>,
  },
  actions: set => {
    const unsubscribe = (runId: string) => {
      subscriptions[runId]?.close()
      delete subscriptions[runId]
    }
    return {
      unsubscribe,
      /** Open (or re-open) the SSE stream for a run. Idempotent per run. */
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
              // Carry the terminal error so a view subscribing AFTER the run
              // failed still renders the run-level error alert.
              v.error = d.error ?? undefined
              v.totalTokens = d.total_tokens
              v.currentStep = d.current_step ?? undefined
              v.pendingElicitation =
                (d.pending_elicitation_json as SSEElicitationRequiredData | undefined) ?? undefined
              // P1: seed the full pipeline (pending steps incl.) up front.
              seedManifest(v, d.step_manifest)
              // P2: rehydrate the running step's in-flight tracks after reconnect.
              if (d.step_progress_json && d.current_step) {
                const s = ensureStep(v, d.current_step)
                s.tracks = d.step_progress_json as Record<string, ProgressTrack>
              }
              // Rehydrate the agent ACTIVITY TIMELINE from durable per-step
              // history so reopening a completed/resumed run replays every row.
              // Persisted as `step_logs_json["<stepId>::agent_activity"]` → an
              // array of `agent_activity` payloads. Array-merge (dedupe by seq).
              const logs = (d.step_logs_json ?? {}) as Record<string, unknown>
              const AGENT_SUFFIX = '::agent_activity'
              for (const [key, value] of Object.entries(logs)) {
                if (!key.endsWith(AGENT_SUFFIX) || !Array.isArray(value)) continue
                const stepId = key.slice(0, -AGENT_SUFFIX.length)
                // Guard the empty derived stepId: a key that is EXACTLY
                // "::agent_activity" would otherwise materialize a phantom
                // `ensureStep(v, '')` step. Skip it (and any array with no
                // well-formed entries).
                if (!stepId) continue
                const wellFormed = (value as AgentActivityEntry[]).filter(
                  raw => raw && typeof raw === 'object' && typeof raw.seq === 'number',
                )
                if (wellFormed.length === 0) continue
                const s = ensureStep(v, stepId)
                if (!s.agentActivity) s.agentActivity = []
                mergeAgentActivityBatch(s.agentActivity, wellFormed)
              }
              // Hydrate per-step output + artifact metadata (metadata only;
              // content fetched lazily via readOutput / readArtifact).
              const outputs = (d.step_outputs_json ?? {}) as Record<string, StepOutputMeta>
              for (const [stepId, meta] of Object.entries(outputs)) {
                const s = ensureStep(v, stepId)
                s.outputMeta = meta
                s.hasOutput = true
                if (!s.outputPreview && meta?.preview) s.outputPreview = meta.preview
                if (s.status === 'pending') s.status = 'completed'
              }
              const artifacts = (d.step_artifacts_json ?? {}) as Record<string, StepArtifactMeta[]>
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
              // P1: upgrade the label to the full-context render.
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
                // Agent-activity tracks feed the dedicated ACTIVITY TIMELINE, not
                // the generic track map — and the `done`-delete path must NOT
                // apply (a completed activity row stays visible in history).
                if (t.kind.type === 'agent_activity') {
                  if (!s.agentActivity) s.agentActivity = []
                  mergeAgentActivity(s.agentActivity, t.kind)
                  continue
                }
                const id = t.id ?? ''
                // `done` tracks were delivered once + evicted backend-side.
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
              // A completed step has a full output file on disk; flag so the
              // "Show full output" expander mounts.
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
            unsubscribe(runId)
          },
          runCancelled: d => {
            set(draft => {
              const v = draft.runs[runId] ?? blankView(runId)
              v.status = 'cancelled'
              v.totalTokens = d.total_tokens
              draft.runs[runId] = v
            })
            unsubscribe(runId)
          },
          runFailed: d => {
            set(draft => {
              const v = draft.runs[runId] ?? blankView(runId)
              v.status = 'failed'
              v.error = d.error
              v.totalTokens = d.total_tokens
              draft.runs[runId] = v
            })
            unsubscribe(runId)
          },
          disconnected: () => {
            set(draft => {
              const v = draft.runs[runId]
              if (v) v.connected = false
            })
          },
        })
      },
      cancel: async (runId: string) => {
        set(draft => {
          draft.cancelling[runId] = true
        })
        try {
          await ApiClient.Workflow.cancelRun({ run_id: runId })
        } catch (e) {
          // M-7: callers fire-and-forget via `void`, so surface here rather than
          // reject unhandled. Show the failure in the run's error banner.
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
      submitElicitation: async (runId: string, elicitationId: string, response: any) => {
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
    }
  },
})

export const useWorkflowRunStore = WorkflowRun.store
