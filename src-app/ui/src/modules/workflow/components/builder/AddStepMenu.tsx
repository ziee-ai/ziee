import { Plus } from 'lucide-react'
import { Button, Dropdown, type DropdownItem } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import {
  STEP_KINDS,
  STEP_KIND_DESCRIPTIONS,
  STEP_KIND_LABELS,
} from './stepForms'

interface AddStepMenuProps {
  store: WorkflowBuilderStore
}

/** Kind picker for appending a step. The agent kind is surfaced in domain
 *  language ("AI assistant task"); the rest by their name. */
export function AddStepMenu({ store }: AddStepMenuProps) {
  const items: DropdownItem[] = STEP_KINDS.map(kind => ({
    key: kind,
    label: (
      <span className="flex flex-col">
        <span className="text-sm">{STEP_KIND_LABELS[kind]}</span>
        <span className="text-xs text-muted-foreground">
          {STEP_KIND_DESCRIPTIONS[kind]}
        </span>
      </span>
    ),
    onClick: () => store.addStep(kind),
  }))

  return (
    <Dropdown items={items} align="start" data-testid="wf-builder-add-step-menu">
      <Button
        type="button"
        variant="outline"
        icon={<Plus />}
        data-testid="wf-builder-add-step-btn"
      >
        Add step
      </Button>
    </Dropdown>
  )
}
