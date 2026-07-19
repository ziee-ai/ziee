import { ApiClient } from '@/api-client'
import {
  Permissions,
  type InputDef,
  type ValidateDefResponse,
  type Workflow,
  type WorkflowDef,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineLocalStore } from '@ziee/framework/store-kit'
import {
  type BuilderStep,
  type StepKind,
  createStep,
} from '../components/builder/stepForms'

// ---------------------------------------------------------------------------
// PRIVATE, per-instance store backing ONE builder editing session (ITEM-6).
// A builder is an editing session, not a shared singleton — so it uses
// `defineLocalStore`: each mount of the builder page owns its own working
// definition, and its sync listeners auto-unsubscribe on unmount. It is NOT
// registered in `Stores.X` (local stores never are); the page instantiates it
// with `WorkflowBuilderStoreDef.use()` and threads the instance to its child
// panels as a prop.
// ---------------------------------------------------------------------------

/** The working definition. `steps` carries the richer `BuilderStep[]` (the base
 *  fields the generated `StepDef` drops) — assignable back to `WorkflowDef` at
 *  the API boundary. */
export interface BuilderDef {
  $schema?: string
  inputs: InputDef[]
  steps: BuilderStep[]
  max_runtime_secs?: number
}

function emptyDef(): BuilderDef {
  return { inputs: [], steps: [] }
}

function toBuilderDef(def: WorkflowDef): BuilderDef {
  return {
    $schema: def.$schema,
    max_runtime_secs: def.max_runtime_secs,
    inputs: def.inputs ?? [],
    // The wire steps already carry `id` + base fields at runtime (the backend
    // sends them); only the generated TS type is lossy, so this cast is sound.
    steps: (def.steps ?? []) as unknown as BuilderStep[],
  }
}

function toWorkflowDef(def: BuilderDef): WorkflowDef {
  return {
    ...(def.$schema ? { $schema: def.$schema } : {}),
    ...(def.max_runtime_secs != null
      ? { max_runtime_secs: def.max_runtime_secs }
      : {}),
    inputs: def.inputs,
    steps: def.steps as unknown as WorkflowDef['steps'],
  }
}

const VALIDATE_DEBOUNCE_MS = 400

export const WorkflowBuilderStoreDef = defineLocalStore({
  immer: true,
  state: {
    /** null in create mode until saved; the workflow id in edit mode. */
    workflowId: null as string | null,
    /** Workflow name — only submitted on create (definition PUT preserves it). */
    name: '' as string,
    def: emptyDef() as BuilderDef,
    dirty: false,
    selectedStepId: null as string | null,
    validation: null as ValidateDefResponse | null,
    validating: false,
    saving: false,
    loading: false,
    loadError: null as string | null,
    error: null as string | null,
    /** Flipped when the workflow being edited is deleted on another device. */
    deletedExternally: false,
  },

  actions: (set, get) => {
    // Per-instance debounce handle (the actions closure is built once per store
    // instance, so this timer never leaks across concurrent builders).
    let validateTimer: ReturnType<typeof setTimeout> | null = null

    const runValidate = async () => {
      const def = get().def
      set(d => {
        d.validating = true
      })
      try {
        const result = await ApiClient.Workflow.validateDef(toWorkflowDef(def))
        set(d => {
          d.validation = result
          d.validating = false
        })
      } catch (error) {
        set(d => {
          d.validating = false
          // Keep the prior validation rather than blanking it on a transient
          // failure; surface the error so Save isn't silently stuck.
          d.error =
            error instanceof Error ? error.message : 'Failed to validate workflow'
        })
      }
    }

    const scheduleValidate = () => {
      if (validateTimer) clearTimeout(validateTimer)
      validateTimer = setTimeout(() => {
        validateTimer = null
        void runValidate()
      }, VALIDATE_DEBOUNCE_MS)
    }

    const detectExternalChanges = async () => {
      const id = get().workflowId
      if (!id) return
      if (!hasPermissionNow(Permissions.WorkflowsRead)) return
      try {
        const def = await ApiClient.Workflow.getDefinition({ id })
        // Only refresh from the server when the author has no unsaved edits, so
        // a cross-device refetch never clobbers in-progress work.
        if (!get().dirty) {
          set(d => {
            d.def = toBuilderDef(def)
          })
        }
      } catch {
        set(d => {
          d.deletedExternally = true
        })
      }
    }

    return {
      /** Start a blank create session. */
      initEmpty: () => {
        set(d => {
          d.workflowId = null
          d.name = ''
          d.def = emptyDef()
          d.dirty = false
          d.selectedStepId = null
          d.validation = null
          d.loadError = null
          d.error = null
          d.deletedExternally = false
        })
      },

      /** Load an existing workflow's definition into an edit session. */
      load: async (id: string) => {
        if (!hasPermissionNow(Permissions.WorkflowsRead)) return
        set(d => {
          d.loading = true
          d.loadError = null
        })
        try {
          const def = await ApiClient.Workflow.getDefinition({ id })
          const builderDef = toBuilderDef(def)
          set(d => {
            d.workflowId = id
            d.def = builderDef
            d.dirty = false
            d.loading = false
            d.selectedStepId = builderDef.steps[0]?.id ?? null
            d.deletedExternally = false
          })
          void runValidate()
        } catch (error) {
          set(d => {
            d.loading = false
            d.loadError =
              error instanceof Error
                ? error.message
                : 'Failed to load workflow definition'
          })
        }
      },

      setName: (name: string) => {
        set(d => {
          d.name = name
          d.dirty = true
        })
      },

      selectStep: (id: string | null) => {
        set(d => {
          d.selectedStepId = id
        })
      },

      addStep: (kind: StepKind) => {
        set(d => {
          const step = createStep(
            kind,
            d.def.steps.map(s => s.id),
          )
          d.def.steps.push(step)
          d.selectedStepId = step.id
          d.dirty = true
        })
        scheduleValidate()
      },

      updateStep: (id: string, patch: Record<string, unknown>) => {
        set(d => {
          const step = d.def.steps.find(s => s.id === id)
          if (!step) return
          Object.assign(step, patch)
          d.dirty = true
        })
        scheduleValidate()
      },

      reorderStep: (from: number, to: number) => {
        set(d => {
          const steps = d.def.steps
          if (
            from < 0 ||
            from >= steps.length ||
            to < 0 ||
            to >= steps.length ||
            from === to
          ) {
            return
          }
          const [moved] = steps.splice(from, 1)
          steps.splice(to, 0, moved)
          d.dirty = true
        })
        scheduleValidate()
      },

      deleteStep: (id: string) => {
        set(d => {
          d.def.steps = d.def.steps.filter(s => s.id !== id)
          if (d.selectedStepId === id) {
            d.selectedStepId = d.def.steps[0]?.id ?? null
          }
          d.dirty = true
        })
        scheduleValidate()
      },

      updateInputs: (inputs: InputDef[]) => {
        set(d => {
          d.def.inputs = inputs
          d.dirty = true
        })
        scheduleValidate()
      },

      /** Debounced validation — called by every mutation. */
      scheduleValidate,
      /** Immediate validation (used on mount / before save). */
      validate: runValidate,
      /** Re-check the edited workflow's server-side existence (cross-device
       *  delete detection); refreshes the def when there are no local edits. */
      detectExternalChanges,

      /** Persist: POST on create, PUT-in-place on edit (id preserved). Returns
       *  the saved workflow. Throws on failure (callers show a toast). */
      save: async (): Promise<Workflow> => {
        const { workflowId, name, def } = get()
        if (!workflowId && !name.trim()) {
          const msg = 'Give the workflow a name before saving'
          set(d => {
            d.error = msg
          })
          throw new Error(msg)
        }
        set(d => {
          d.saving = true
          d.error = null
        })
        try {
          const payload = toWorkflowDef(def)
          let workflow: Workflow
          if (workflowId) {
            workflow = await ApiClient.Workflow.updateDefinition({
              id: workflowId,
              ...payload,
            })
          } else {
            workflow = await ApiClient.Workflow.create({
              name: name.trim(),
              ...payload,
            })
          }
          set(d => {
            d.workflowId = workflow.id
            d.dirty = false
            d.saving = false
          })
          return workflow
        } catch (error) {
          set(d => {
            d.saving = false
            d.error =
              error instanceof Error ? error.message : 'Failed to save workflow'
          })
          throw error
        }
      },
    }
  },

  // Runs on MOUNT; listeners auto-unsubscribe on UNMOUNT.
  init: ({ on, get, set, actions }) => {
    on('sync:workflow', event => {
      const { action, id } = event.data
      if (id !== get().workflowId) return
      if (action === 'delete') {
        set(d => {
          d.deletedExternally = true
        })
        return
      }
      // A non-delete change to OUR workflow on another device — reconcile.
      void actions.detectExternalChanges()
    })
    on('sync:reconnect', () => {
      // Detect a delete we may have missed while disconnected. `detectExternalChanges`
      // self-gates on the read permission (the refetch endpoint enforces it).
      if (get().workflowId) void actions.detectExternalChanges()
    })
  },
})

export type WorkflowBuilderStore = ReturnType<typeof WorkflowBuilderStoreDef.use>
