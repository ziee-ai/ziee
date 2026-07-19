import { useRef } from 'react'
import { ChevronDown, ChevronUp, GripVertical, Trash2 } from 'lucide-react'
import { Button, Empty, Tag, Text } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { AddStepMenu } from './AddStepMenu'
import { STEP_KIND_LABELS, type StepKind } from './stepForms'

interface StepListProps {
  store: WorkflowBuilderStore
}

/**
 * ITEM-7 — the ordered, reorderable step list (master column). Steps reorder by
 * native drag-and-drop AND accessible up/down buttons (the app has no shared
 * reorder-DnD idiom, and buttons keep it keyboard-usable + reliably testable).
 * Selecting a row drives the detail config panel.
 */
export function StepList({ store }: StepListProps) {
  const steps = store.def.steps
  const selectedStepId = store.selectedStepId
  const dragIndex = useRef<number | null>(null)

  const onDrop = (dropIndex: number) => {
    const from = dragIndex.current
    dragIndex.current = null
    if (from == null) return
    store.reorderStep(from, dropIndex)
  }

  return (
    <div className="flex flex-col gap-3" data-testid="wf-builder-step-list">
      <div className="flex items-center justify-between gap-2">
        <Text strong>Steps</Text>
        <AddStepMenu store={store} />
      </div>

      {steps.length === 0 ? (
        <Empty
          data-testid="wf-builder-steps-empty"
          description="No steps yet — add one to begin"
        />
      ) : (
        <ol className="flex flex-col gap-2 ps-0">
          {steps.map((step, i) => {
            const selected = step.id === selectedStepId
            return (
              <li
                key={step.id}
                draggable
                onDragStart={() => {
                  dragIndex.current = i
                }}
                onDragOver={e => e.preventDefault()}
                onDrop={() => onDrop(i)}
                data-testid={`wf-builder-step-row-${step.id}`}
                data-selected={selected || undefined}
                className={`flex items-center gap-2 rounded-md border p-2 cursor-pointer ${
                  selected
                    ? 'border-primary bg-accent'
                    : 'border-border hover:bg-accent/50'
                }`}
                onClick={() => store.selectStep(step.id)}
              >
                <GripVertical
                  className="size-4 shrink-0 text-muted-foreground cursor-grab"
                  aria-hidden
                />
                <div className="flex min-w-0 flex-1 flex-col">
                  <div className="flex items-center gap-2 min-w-0">
                    <Text className="text-xs text-muted-foreground shrink-0">
                      {i + 1}.
                    </Text>
                    <Text className="truncate text-sm" strong={selected}>
                      {step.description?.trim() || step.id}
                    </Text>
                  </div>
                  <Tag
                    variant="outline"
                    tone="info"
                    className="text-xs !m-0 self-start"
                    data-testid={`wf-builder-step-kind-${step.id}`}
                  >
                    {STEP_KIND_LABELS[step.kind as StepKind] ?? step.kind}
                  </Tag>
                </div>
                <div className="flex shrink-0 items-center">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    icon={<ChevronUp />}
                    aria-label="Move step up"
                    data-testid={`wf-builder-step-up-${step.id}`}
                    disabled={i === 0}
                    onClick={e => {
                      e.stopPropagation()
                      store.reorderStep(i, i - 1)
                    }}
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    icon={<ChevronDown />}
                    aria-label="Move step down"
                    data-testid={`wf-builder-step-down-${step.id}`}
                    disabled={i === steps.length - 1}
                    onClick={e => {
                      e.stopPropagation()
                      store.reorderStep(i, i + 1)
                    }}
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    icon={<Trash2 />}
                    aria-label="Delete step"
                    data-testid={`wf-builder-step-delete-${step.id}`}
                    onClick={e => {
                      e.stopPropagation()
                      store.deleteStep(step.id)
                    }}
                  />
                </div>
              </li>
            )
          })}
        </ol>
      )}
    </div>
  )
}
