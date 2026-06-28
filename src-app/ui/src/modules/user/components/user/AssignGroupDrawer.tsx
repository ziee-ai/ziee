import { Button, Flex, Form, FormField, useForm, zodResolver, Select, message } from '@/components/ui'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const assignGroupSchema = z.object({
  group_id: z.string().min(1, 'Please select a group'),
})
type AssignGroupValues = z.infer<typeof assignGroupSchema>

export function AssignGroupDrawer() {
  const { isOpen, user } = Stores.AssignGroupDrawer
  const { groups } = Stores.UserGroups
  const assignGroupForm = useForm<AssignGroupValues>({
    resolver: zodResolver(assignGroupSchema),
    defaultValues: { group_id: '' },
  })
  const canAssign = usePermission(Permissions.GroupsAssignUsers)

  const handleAssignGroup = async (values: AssignGroupValues) => {
    if (!user) return

    try {
      await Stores.UserGroups.assignUserToUserGroup(user.id, values.group_id)
      message.success('User assigned to group successfully')
      Stores.AssignGroupDrawer.closeAssignGroupDrawer()
      assignGroupForm.reset()
    } catch (error) {
      console.error('Failed to assign user to group:', error)
      // Error is handled by the store
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
      footer={null}
      mask={{ closable: false }}
    >
      <Form
        form={assignGroupForm}
        layout="vertical"
        onSubmit={handleAssignGroup}
        disabled={!canAssign}
        data-testid="user-assign-group-form"
      >
        <FormField name="group_id" label="Select Group" required>
          <Select
            placeholder="Select group to assign"
            options={groups.map(group => ({ value: group.id, label: group.name }))}
            data-testid="user-assign-group-select"
          />
        </FormField>
        <Flex className="justify-end gap-2">
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
            <Button type="submit" data-testid="user-assign-group-submit-button">
              Assign
            </Button>
          )}
        </Flex>
      </Form>
    </Drawer>
  )
}
