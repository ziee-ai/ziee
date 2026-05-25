import { App, Button, Flex, Form, Select } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'

const { Option } = Select

export function AssignGroupDrawer() {
  const { message } = App.useApp()
  const { isOpen, user } = Stores.AssignGroupDrawer
  const { groups } = Stores.UserGroups
  const [assignGroupForm] = Form.useForm()
  const canAssign = usePermission('groups::assign_users')

  const handleAssignGroup = async (values: any) => {
    if (!user) return

    try {
      await Stores.UserGroups.assignUserToUserGroup(user.id, values.group_id)
      message.success('User assigned to group successfully')
      Stores.AssignGroupDrawer.closeAssignGroupDrawer()
      assignGroupForm.resetFields()
    } catch (error) {
      console.error('Failed to assign user to group:', error)
      // Error is handled by the store
    }
  }

  return (
    <Drawer
      title="Assign to Group"
      open={isOpen}
      onClose={() => {
        Stores.AssignGroupDrawer.closeAssignGroupDrawer()
        assignGroupForm.resetFields()
      }}
      footer={null}
      maskClosable={false}
    >
      <Form
        form={assignGroupForm}
        layout="vertical"
        onFinish={handleAssignGroup}
        disabled={!canAssign}
      >
        <Form.Item
          name="group_id"
          label="Select Group"
          rules={[
            {
              required: true,
              message: 'Please select a group',
            },
          ]}
        >
          <Select placeholder="Select group to assign">
            {groups.map(group => (
              <Option key={group.id} value={group.id}>
                {group.name}
              </Option>
            ))}
          </Select>
        </Form.Item>
        <Form.Item className="mb-0">
          <Flex className="gap-2">
            {canAssign && (
              <Button type="primary" htmlType="submit">
                Assign Group
              </Button>
            )}
            <Button
              onClick={() => {
                Stores.AssignGroupDrawer.closeAssignGroupDrawer()
                assignGroupForm.resetFields()
              }}
            >
              {canAssign ? 'Cancel' : 'Close'}
            </Button>
          </Flex>
        </Form.Item>
      </Form>
    </Drawer>
  )
}
