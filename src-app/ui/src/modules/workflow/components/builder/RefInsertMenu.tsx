import { Braces } from 'lucide-react'
import { Button, Dropdown, type DropdownItem } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { enumerateRefs } from './refInsert'

interface RefInsertMenuProps {
  store: WorkflowBuilderStore
  /** The step whose field is being edited (only PRIOR steps are referenceable). */
  stepId: string
  /** Insert the chosen reference token into the target field. */
  onInsert: (token: string) => void
  testid?: string
}

/**
 * ITEM-10 — a menu listing the workflow's inputs + every PRIOR step's output,
 * with a type hint per entry. Selecting one inserts the correct reference token
 * (`{{ inputs.x }}` / `{{ step_id.output }}`) into the field. The valid-ref
 * enumeration lives in the pure `enumerateRefs` helper.
 */
export function RefInsertMenu({
  store,
  stepId,
  onInsert,
  testid = 'wf-builder-ref-menu',
}: RefInsertMenuProps) {
  const def = store.def
  const index = def.steps.findIndex(s => s.id === stepId)
  const refs = enumerateRefs(def, index)

  const items: DropdownItem[] = []
  if (refs.length === 0) {
    items.push({
      key: 'none',
      label: 'No references available yet',
      disabled: true,
    })
  } else {
    let lastGroup: string | null = null
    for (const ref of refs) {
      if (ref.group !== lastGroup) {
        items.push({ type: 'label', label: ref.group })
        lastGroup = ref.group
      }
      items.push({
        key: ref.token,
        label: (
          <span className="flex items-center gap-2">
            <span className="truncate">{ref.label}</span>
            {ref.hint && (
              <span className="text-xs text-muted-foreground">{ref.hint}</span>
            )}
          </span>
        ),
        onClick: () => onInsert(ref.token),
      })
    }
  }

  return (
    <Dropdown items={items} align="end" data-testid={testid}>
      <Button
        type="button"
        variant="ghost"
        icon={<Braces />}
        data-testid={`${testid}-trigger`}
      >
        Insert reference
      </Button>
    </Dropdown>
  )
}
