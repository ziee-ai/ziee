import { App, Button, Checkbox, Flex, Form } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'


export function AssignGroupDrawer() {
  const { message } = App.useApp()
  const { isOpen, user } = Stores.AssignGroupDrawer
  const { groups } = Stores.UserGroups
  const [assignGroupForm] = Form.useForm()
  const canAssign = usePermission(Permissions.GroupsAssignUsers)

  const handleAssignGroup = async (values: any) => {
    if (!user) return

    const { group_ids } = values
    if (!group_ids || group_ids.length === 0) return

    try {
      let successCount = 0
      const errors: string[] = []

      for (const groupId of group_ids) {
        try {
          await Stores.UserGroups.assignUserToUserGroup(user.id, groupId)
          successCount++
        } catch (error) {
          const group = groups.find(g => g.id === groupId)
          const name = group?.name ?? groupId
          errors.push(`"${name}": ${error instanceof Error ? error.message : 'Unknown error'}`)
        }
      }

      if (successCount > 0) {
        message.success(`User assigned to ${successCount} group(s) successfully`)
      }
      errors.forEach(err => message.error(err))

      Stores.AssignGroupDrawer.closeAssignGroupDrawer()
      assignGroupForm.resetFields()
    } catch (error) {
      console.error('Failed to assign user to group:', error)
    }
  }

  return (
    <Drawer
      title="Assign to Group"
      size={600}
      open={isOpen}
      onClose={() => {
        Stores.AssignGroupDrawer.closeAssignGroupDrawer()
        assignGroupForm.resetFields()
      }}
      footer={null}
      mask={{ closable: false }}
    >
      <Form
        form={assignGroupForm}
        layout="vertical"
        onFinish={handleAssignGroup}
        disabled={!canAssign}
      >
        <Form.Item
          name="group_ids"
          label="Select Group"
          rules={[
            {
              validator: (_, value: string[]) => {
                if (!value || value.length === 0) {
                  return Promise.reject(new Error('Please select at least one group'))
                }
                return Promise.resolve()
              },
            },
          ]}
        >
          <Checkbox.Group className="flex flex-col gap-2">
            {groups.map(group => (
              <Checkbox key={group.id} value={group.id}>
                {group.name}
              </Checkbox>
            ))}
          </Checkbox.Group>
        </Form.Item>
        <Form.Item className="mb-0">
          <Flex className="justify-end gap-2">
            <Button
              onClick={() => {
                Stores.AssignGroupDrawer.closeAssignGroupDrawer()
                assignGroupForm.resetFields()
              }}
            >
              {canAssign ? 'Cancel' : 'Close'}
            </Button>
            {canAssign && (
              <Button type="primary" htmlType="submit">
                Assign
              </Button>
            )}
          </Flex>
        </Form.Item>
      </Form>
    </Drawer>
  )
}
