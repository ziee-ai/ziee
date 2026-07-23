import type {
  ProgressTrack,
  SSEElicitationRequiredData,
} from '@/api-client/types'
import type { WorkflowRunGet, WorkflowRunSet } from '../state'
import { blankView, ensureStep, seedManifest, subscriptions } from '../state'
import { mergeAgentActivity, mergeAgentActivityBatch } from '../agentActivity'
import type { AgentActivityEntry } from '@/modules/workflow/components/run/activityDescriptors'
import { subscribeRunProgress } from '@/modules/workflow/sse/runProgressClient'

export default (set: WorkflowRunSet, _get: WorkflowRunGet) => {
  return (runId: string) => {
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
          v.error = d.error ?? undefined
          v.totalTokens = d.total_tokens
          v.currentStep = d.current_step ?? undefined
          v.pendingElicitation =
            (d.pending_elicitation_json as SSEElicitationRequiredData | undefined) ?? undefined
          seedManifest(v, d.step_manifest)
          if (d.step_progress_json && d.current_step) {
            const s = ensureStep(v, d.current_step)
            s.tracks = d.step_progress_json as Record<string, ProgressTrack>
          }
          const outputs = (d.step_outputs_json ?? {}) as Record<string, any>
          for (const [stepId, meta] of Object.entries(outputs)) {
            const s = ensureStep(v, stepId)
            s.outputMeta = meta
            s.hasOutput = true
            if (!s.outputPreview && meta?.preview) s.outputPreview = meta.preview
            if (s.status === 'pending') s.status = 'completed'
          }
          const artifacts = (d.step_artifacts_json ?? {}) as Record<string, any[]>
          for (const [stepId, list] of Object.entries(artifacts)) {
            if (!Array.isArray(list) || list.length === 0) continue
            const s = ensureStep(v, stepId)
            s.artifacts = list
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
            // Agent-activity tracks feed the dedicated ACTIVITY TIMELINE, not
            // the generic track map — and the `done`-delete path must NOT
            // apply (a completed activity row stays visible in history).
            if (t.kind.type === 'agent_activity') {
              if (!s.agentActivity) s.agentActivity = []
              mergeAgentActivity(s.agentActivity, t.kind)
              continue
            }
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
        subscriptions[runId]?.close()
        delete subscriptions[runId]
      },
      runCancelled: d => {
        set(draft => {
          const v = draft.runs[runId] ?? blankView(runId)
          v.status = 'cancelled'
          v.totalTokens = d.total_tokens
          draft.runs[runId] = v
        })
        subscriptions[runId]?.close()
        delete subscriptions[runId]
      },
      runFailed: d => {
        set(draft => {
          const v = draft.runs[runId] ?? blankView(runId)
          v.status = 'failed'
          v.error = d.error
          v.totalTokens = d.total_tokens
          draft.runs[runId] = v
        })
        subscriptions[runId]?.close()
        delete subscriptions[runId]
      },
      disconnected: () => {
        set(draft => {
          const v = draft.runs[runId]
          if (v) v.connected = false
        })
      },
    })
  }
}
