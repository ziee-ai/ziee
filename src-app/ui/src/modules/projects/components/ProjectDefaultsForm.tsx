import { useCallback, useEffect, useRef, useState } from 'react'
import { Combobox, Loading, message } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { type Assistant, type LlmModel, type Project, type UpdateProjectRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'

interface ProjectDefaultsFormProps {
  project: Project
}

/**
 * Inline auto-save form for a project's Default assistant + Default
 * model. Lives in the ProjectDetailPage's Advanced card.
 *
 * Why this is a separate component (not part of ProjectFormDrawer):
 *  - These are configuration-shape settings (a foreign-key pick that
 *    snapshots onto every new conversation), not content like name /
 *    description / instructions. Keeping them inline on the page
 *    means a small change doesn't require opening + saving + closing
 *    the drawer.
 *  - Auto-save on change matches how the user thinks about it ("set
 *    a default" feels atomic, not transactional). Each select fires
 *    one PATCH; the surrounding form's submit button doesn't exist.
 *
 * Tri-state on default_*_id (backend Option<Option<Uuid>>):
 *  - undefined → field omitted → "no change"
 *  - null      → JSON `null`   → "clear"
 *  - string    → uuid          → "set"
 * The generated TS types flatten the outer Option to
 * `string | undefined`, so we cast through `unknown` to wire null
 * through when the user clears a picker. The backend's
 * `deserialize_nullable_field` reads it correctly.
 */
export function ProjectDefaultsForm({ project }: ProjectDefaultsFormProps) {
  const canEdit = usePermission(Permissions.ProjectsEdit)

  // Picker option lists. Loaded once on mount + refetched when an
  // assistant/model lifecycle event lands.
  const [assistants, setAssistants] = useState<Assistant[]>([])
  const [models, setModels] = useState<LlmModel[]>([])
  const [optionsLoading, setOptionsLoading] = useState(false)
  // Distinguishes the first fetch (show a skeleton — the pickers have no
  // value yet) from background refetches (keep the current value visible +
  // just lock the picker via the Combobox `loading` prop).
  const [loadedOnce, setLoadedOnce] = useState(false)

  // Per-field saving spinner so the two selects don't share a loader
  // (clicking model shouldn't grey out assistant).
  const [savingAssistant, setSavingAssistant] = useState(false)
  const [savingModel, setSavingModel] = useState(false)

  // Mount guard for late-landing fetches.
  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const refetchOptions = useCallback(async () => {
    if (!mountedRef.current) return
    setOptionsLoading(true)
    try {
      const [assistantsResp, modelsResp] = await Promise.all([
        ApiClient.Assistant.list({ page: 1, limit: 100 }),
        ApiClient.LlmModel.list({ page: 1, perPage: 100 }),
      ])
      if (!mountedRef.current) return
      setAssistants(assistantsResp.assistants ?? [])
      setModels(modelsResp.models ?? [])
    } catch (err) {
      console.warn('Failed to load default-asset options', err)
      if (mountedRef.current) {
        message.error('Failed to load assistant/model options')
      }
    } finally {
      if (mountedRef.current) {
        setOptionsLoading(false)
        setLoadedOnce(true)
      }
    }
  }, [])

  useEffect(() => {
    void refetchOptions()
  }, [refetchOptions])

  // Live-refresh options as assets are created/deleted elsewhere in
  // the app — same set of events the drawer used to listen for.
  useEffect(() => {
    const GROUP = 'ProjectDefaultsForm'
    const eventBus = Stores.EventBus
    const offs = [
      eventBus.on('assistant.created', () => void refetchOptions(), GROUP),
      eventBus.on('assistant.deleted', () => void refetchOptions(), GROUP),
      eventBus.on('assistant.updated', () => void refetchOptions(), GROUP),
      eventBus.on('llm_model.enabled', () => void refetchOptions(), GROUP),
      eventBus.on('llm_model.disabled', () => void refetchOptions(), GROUP),
      eventBus.on('llm_model.deleted', () => void refetchOptions(), GROUP),
    ]
    return () => offs.forEach(off => off())
  }, [refetchOptions])

  const handleAssistantChange = async (value: string) => {
    // Empty string from Combobox means "clear" → send null to the API
    const resolved = value === '' ? null : value
    setSavingAssistant(true)
    try {
      const patch: UpdateProjectRequest = {
        default_assistant_id: (resolved ?? null) as unknown as string | undefined,
      }
      await Stores.Projects.updateProject(project.id, patch)
      message.success('Default assistant updated')
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to update default assistant',
      )
    } finally {
      setSavingAssistant(false)
    }
  }

  const handleModelChange = async (value: string) => {
    // Empty string from Combobox means "clear" → send null to the API
    const resolved = value === '' ? null : value
    setSavingModel(true)
    try {
      const patch: UpdateProjectRequest = {
        default_model_id: (resolved ?? null) as unknown as string | undefined,
      }
      await Stores.Projects.updateProject(project.id, patch)
      message.success('Default model updated')
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to update default model',
      )
    } finally {
      setSavingModel(false)
    }
  }

  const assistantOptions = (() => {
    // "No default" sentinel lets users clear the selection (Combobox has no allowClear)
    const base = [{ value: '', label: 'No default' }]
    const opts = assistants.map(a => ({
      value: a.id as string,
      label: a.name,
    }))
    // Preserve the current selection even if it was deleted upstream,
    // so the user still sees what's set + can explicitly clear it.
    const current = project.default_assistant_id
    if (current && !opts.some(o => o.value === current)) {
      opts.unshift({ value: current, label: '(deleted)' })
    }
    return [...base, ...opts]
  })()

  const modelOptions = (() => {
    // "No default" sentinel lets users clear the selection (Combobox has no allowClear)
    const base = [{ value: '', label: 'No default' }]
    const opts = models.map(m => ({
      value: m.id as string,
      label: m.display_name || m.name,
    }))
    const current = project.default_model_id
    if (current && !opts.some(o => o.value === current)) {
      opts.unshift({ value: current, label: '(deleted)' })
    }
    return [...base, ...opts]
  })()

  // On the very first fetch the pickers have no value to show yet, so
  // present a skeleton instead of a disabled "No default" placeholder
  // (which reads as "there are no options" rather than "still loading").
  const showInitialSkeleton = optionsLoading && !loadedOnce

  // Wrapper data-test-* attributes carry the boolean "is a default
  // set?" signal used by the project detail-page E2E specs to assert
  // the Advanced section's summary state without scraping the antd
  // Select's value (which is just a UUID).
  const body = (
    <div className="flex flex-col gap-4">
        <div
          data-test-default-assistant-set={project.default_assistant_id ? 'true' : 'false'}
          className="flex flex-col gap-1.5"
        >
          <label className="text-sm font-medium leading-none">Default assistant</label>
          <Combobox
            data-testid="project-default-assistant-combobox"
            placeholder="No default"
            searchPlaceholder="Search assistants…"
            emptyText="No assistants found"
            loading={optionsLoading || savingAssistant}
            value={project.default_assistant_id ?? ''}
            onChange={value => void handleAssistantChange(value)}
            options={assistantOptions}
          />
          <p className="text-sm text-muted-foreground">
            Pre-selected when creating a new conversation in this project. Users can override per-conversation.
          </p>
        </div>

        <div
          data-test-default-model-set={project.default_model_id ? 'true' : 'false'}
          className="flex flex-col gap-1.5"
        >
          <label className="text-sm font-medium leading-none">Default model</label>
          <Combobox
            data-testid="project-default-model-combobox"
            placeholder="No default"
            searchPlaceholder="Search models…"
            emptyText="No models found"
            loading={optionsLoading || savingModel}
            value={project.default_model_id ?? ''}
            onChange={value => void handleModelChange(value)}
            options={modelOptions}
          />
          <p className="text-sm text-muted-foreground">
            Snapshotted onto each conversation created in this project (when no explicit model is selected).
          </p>
        </div>
      </div>
  )

  return (
    <fieldset disabled={!canEdit} className="contents">
      {showInitialSkeleton ? <Loading>{body}</Loading> : body}
    </fieldset>
  )
}
