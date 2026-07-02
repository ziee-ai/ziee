import { Button, Checkbox, Flex, Form, FormField, useForm, zodResolver, message } from '@/components/ui'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const assignGroupSchema = z.object({
  group_ids: z.array(z.string()).min(1, 'Please select at least one group'),
})
type AssignGroupValues = z.infer<typeof assignGroupSchema>

/**
 * Multi-checkbox group bound to react-hook-form via FormField (which clones
 * `value`/`onChange` onto this single child). Renders one kit Checkbox per
 * group and emits the selected ids as a string[].
 */
function GroupCheckboxes({
  options,
  value,
  onChange,
  disabled,
  'data-testid': testid,
}: {
  options: { value: string; label: string }[]
  value?: string[]
  onChange?: (next: string[]) => void
  disabled?: boolean
  'data-testid': string
}) {
  const selected = value ?? []
  const toggle = (id: string, checked: boolean) => {
    onChange?.(checked ? [...selected, id] : selected.filter(x => x !== id))
  }
  return (
    <Flex className="flex-col gap-2" data-testid={testid}>
      {options.map(o => (
        <Checkbox
          key={o.value}
          checked={selected.includes(o.value)}
          onCheckedChange={(c: boolean) => toggle(o.value, c)}
          disabled={disabled}
          label={o.label}
          data-testid={`${testid}-opt-${o.value}`}
        />
      ))}
    </Flex>
  )
}

export function AssignGroupDrawer() {
  const { isOpen, user } = Stores.AssignGroupDrawer
  const { groups } = Stores.UserGroups
  const assignGroupForm = useForm<AssignGroupValues>({
    resolver: zodResolver(assignGroupSchema),
    defaultValues: { group_ids: [] },
  })
  const canAssign = usePermission(Permissions.GroupsAssignUsers)

  const handleAssignGroup = async (values: AssignGroupValues) => {
    if (!user) return
    const groupIds = values.group_ids
    if (groupIds.length === 0) return

    // Assign each selected group independently, aggregating per-group
    // outcomes so one failure doesn't hide the successes (or vice-versa).
    let successCount = 0
    const errors: string[] = []
    for (const groupId of groupIds) {
      try {
        await Stores.UserGroups.assignUserToUserGroup(user.id, groupId)
        successCount++
      } catch (error) {
        const name = groups.find(g => g.id === groupId)?.name ?? groupId
        errors.push(
          `"${name}": ${error instanceof Error ? error.message : 'Unknown error'}`,
        )
      }
    }

    if (successCount > 0) {
      message.success(`User assigned to ${successCount} group(s) successfully`)
    }
    errors.forEach(err => message.error(err))

    if (errors.length === 0) {
      Stores.AssignGroupDrawer.closeAssignGroupDrawer()
      assignGroupForm.reset()
    }
  }

  return (
    <Drawer
      title="Assign to Group"
      size={600}
      open={isOpen}
      onClose={() => {
        Stores.AssignGroupDrawer.closeAssignGroupDrawer()
        assignGroupForm.reset()
      }}
      footer={
        <div className="flex justify-end gap-2">
          <Button
            variant="outline"
            onClick={() => {
              Stores.AssignGroupDrawer.closeAssignGroupDrawer()
              assignGroupForm.reset()
            }}
            data-testid="user-assign-group-cancel-button"
          >
            {canAssign ? 'Cancel' : 'Close'}
          </Button>
          {canAssign && (
            <Button type="submit" form="assign-group-form" data-testid="user-assign-group-submit-button">
              Assign
            </Button>
          )}
        </div>
      }
      mask={{ closable: false }}
    >
      <Form
        name="assign-group-form"
        form={assignGroupForm}
        layout="vertical"
        onSubmit={handleAssignGroup}
        disabled={!canAssign}
        data-testid="user-assign-group-form"
      >
        <FormField name="group_ids" label="Select Group" required>
          <GroupCheckboxes
            options={groups.map(group => ({ value: group.id, label: group.name }))}
            data-testid="user-assign-group-checkboxes"
          />
        </FormField>
      </Form>
    </Drawer>
  )
}
