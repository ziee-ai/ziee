import { forwardRef } from 'react'

import type { ProviderWithModels } from '@/api-client/types'
import {
  Combobox,
  MultiSelect,
  Select,
  type SelectOptionGroup,
} from '@/components/ui'
import { Stores } from '@/core/stores'

/**
 * Human-readable pickers for a scheduled task's target (ITEM-1 / FB-2). Each
 * picker is a CONTROLLED wrapper bound to the drawer form (value/onChange) — it
 * NEVER mutates the global picker stores (no `selectAssistant`/`setModelId`),
 * it only reads their cached lists to render NAMES instead of raw UUIDs.
 *
 * The wrappers are `forwardRef` + accept the exact prop surface `FormField`
 * injects (value/onChange/onBlur/id/name/invalid/aria-*), so they drop straight
 * into a `<FormField>` and bind automatically.
 */

/** Grouped model options from the user's accessible providers. Pure + exported
 *  so it's unit-testable independent of the store. Mirrors WorkflowRunDialog. */
export function buildModelOptions(
  providers: ProviderWithModels[] | undefined,
): SelectOptionGroup[] {
  return (providers || [])
    .map(p => ({
      label: p.name,
      options: (p.llm_models || [])
        .filter(m => m.enabled)
        .map(m => ({ label: m.display_name || m.name, value: m.id })),
    }))
    .filter(g => g.options.length > 0)
}

interface StringControlProps {
  value?: string
  onChange?: (v: string) => void
  onBlur?: () => void
  id?: string
  name?: string
  invalid?: boolean
  'aria-describedby'?: string
  'aria-labelledby'?: string
  'aria-required'?: boolean
}

/** Assistant picker (Combobox by name). Offers a "Default assistant" option
 *  whose value is the empty string ⇒ `assistant_id` is omitted from the body. */
export const AssistantField = forwardRef<HTMLInputElement, StringControlProps>(
  function AssistantField(
    { value, onChange, onBlur, id, name, invalid, ...aria },
    ref,
  ) {
    const { availableAssistants } = Stores.AssistantPicker
    const options = [
      { label: 'Default assistant', value: '' },
      ...availableAssistants.map(a => ({ label: a.name, value: a.id })),
    ]
    return (
      <Combobox
        ref={ref}
        data-testid="task-form-assistant"
        value={value ?? ''}
        onChange={v => onChange?.(v)}
        onBlur={onBlur}
        id={id}
        name={name}
        invalid={invalid}
        options={options}
        placeholder="Default assistant"
        emptyText="No assistants available"
        searchPlaceholder="Search assistants"
        aria-describedby={aria['aria-describedby']}
        aria-labelledby={aria['aria-labelledby']}
      />
    )
  },
)

/** Workflow picker (Combobox by name) over the user's accessible workflows. */
export const WorkflowField = forwardRef<HTMLInputElement, StringControlProps>(
  function WorkflowField(
    { value, onChange, onBlur, id, name, invalid, ...aria },
    ref,
  ) {
    const { workflows } = Stores.Workflow
    const options = workflows.map(w => ({
      label: w.display_name || w.name,
      value: w.id,
    }))
    return (
      <Combobox
        ref={ref}
        data-testid="task-form-workflow"
        value={value ?? ''}
        onChange={v => onChange?.(v)}
        onBlur={onBlur}
        id={id}
        name={name}
        invalid={invalid}
        options={options}
        placeholder="Select a workflow"
        emptyText="No workflows available"
        searchPlaceholder="Search workflows"
        aria-describedby={aria['aria-describedby']}
        aria-labelledby={aria['aria-labelledby']}
      />
    )
  },
)

/** Model picker (grouped Select) over the user's accessible providers/models. */
export const ModelField = forwardRef<HTMLButtonElement, StringControlProps>(
  function ModelField(
    { value, onChange, onBlur, id, name, invalid, ...aria },
    ref,
  ) {
    const { providers, loading } = Stores.ModelPicker
    const options = buildModelOptions(providers)
    return (
      <Select
        ref={ref}
        data-testid="task-form-model"
        value={value ?? ''}
        onChange={v => onChange?.(v)}
        onBlur={onBlur}
        id={id}
        name={name}
        invalid={invalid}
        options={options}
        loading={loading && options.length === 0}
        placeholder={
          loading && options.length === 0 ? 'Loading models…' : 'Select a model'
        }
        popupMatchSelectWidth={false}
        aria-describedby={aria['aria-describedby']}
        aria-labelledby={aria['aria-labelledby']}
        aria-required={aria['aria-required']}
      />
    )
  },
)

/** A tool the task is permitted to invoke unattended (ITEM-16 / DEC-17.4). */
export interface AllowedUnattendedTool {
  server_id: string
  tool_name?: string
}

interface AllowedToolsControlProps {
  value?: AllowedUnattendedTool[]
  onChange?: (v: AllowedUnattendedTool[]) => void
  onBlur?: () => void
  id?: string
  name?: string
  invalid?: boolean
  'aria-describedby'?: string
}

/** Allow-list picker (ITEM-16). Whole-server granularity — each selected entry
 *  is `{ server_id }` (no `tool_name`), so the task may use every tool on that
 *  server unattended. Adapts between the form's `{ server_id }[]` shape and the
 *  MultiSelect's `string[]` of server ids. Options are the user's MCP servers. */
export const AllowedToolsField = forwardRef<
  HTMLDivElement,
  AllowedToolsControlProps
>(function AllowedToolsField(
  { value, onChange, onBlur, id, name, invalid, ...aria },
  ref,
) {
  const { servers } = Stores.McpServer
  const options = servers.map(s => ({
    label: s.display_name || s.name,
    value: s.id,
  }))
  const selectedIds = (value ?? []).map(t => t.server_id)
  return (
    <MultiSelect
      ref={ref}
      data-testid="task-form-allowed-tools"
      value={selectedIds}
      onChange={ids => onChange?.(ids.map(sid => ({ server_id: sid })))}
      onBlur={onBlur}
      id={id}
      name={name}
      invalid={invalid}
      options={options}
      placeholder="No unattended tools (safe default)"
      searchPlaceholder="Search MCP servers"
      emptyText="No MCP servers available"
      removeLabel={label => `Remove ${label}`}
      aria-label="Tools this task may use unattended"
      aria-describedby={aria['aria-describedby']}
    />
  )
})
