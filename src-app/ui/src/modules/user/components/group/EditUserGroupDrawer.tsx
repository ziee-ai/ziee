import { App, Button, Form, Input, Switch } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type UpdateGroupRequest } from '@/api-client/types'
import { PermissionsField } from '@/modules/user/components/PermissionsField.tsx'

const { TextArea } = Input

export function EditUserGroupDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  const { isOpen: open, editingGroup: group } = Stores.EditUserGroupDrawer
  const canEdit = usePermission(Permissions.GroupsEdit)

  // Load group data when it changes
  useEffect(() => {
    if (group && open) {
      form.setFieldsValue({
        name: group.name,
        description: group.description,
        permissions: group.permissions ?? [],
        is_active: group.is_active,
      })
    }
  }, [group, open, form])

  const handleClose = () => {
    form.resetFields()
    Stores.EditUserGroupDrawer.closeUserGroupDrawer()
  }

  const handleSubmit = async (values: any) => {
    if (!group) return

    try {
      setLoading(true)

      const updateData: UpdateGroupRequest = {
        name: values.name,
        description: values.description,
        permissions: values.permissions ?? [],
        is_active: values.is_active,
      }

      await Stores.UserGroups.updateUserGroup(group.id, updateData)
      message.success('User group updated successfully')
      handleClose()
    } catch (error) {
      console.error('Failed to update user group:', error)
      message.error('Failed to update user group')
    } finally {
      setLoading(false)
    }
  }

  return (
    <Drawer
      title={group ? `Edit Group: ${group.name}` : 'Edit User Group'}
      open={open}
      onClose={handleClose}
      footer={null}
      size={600}
      mask={{ closable: false }}
    >
      <Form
        name="edit-user-group-form"
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
        disabled={!canEdit}
      >
        <Form.Item
          name="name"
          label="Group Name"
          rules={[
            { required: true, message: 'Please enter a group name' },
            { min: 2, message: 'Group name must be at least 2 characters' },
          ]}
        >
          <Input placeholder="Enter group name" />
        </Form.Item>

        <Form.Item name="description" label="Description">
          <TextArea
            placeholder="Enter group description (optional)"
            rows={3}
            showCount
            maxLength={500}
          />
        </Form.Item>

        <Form.Item name="permissions" label="Permissions">
          <PermissionsField disabled={!canEdit} />
        </Form.Item>

        <Form.Item name="is_active" label="Active" valuePropName="checked">
          <Switch aria-label="Set group as active or inactive" />
        </Form.Item>

        <div className="flex justify-end gap-3 pt-4">
          <Button onClick={handleClose} disabled={loading}>
            {canEdit ? 'Cancel' : 'Close'}
          </Button>
          {canEdit && (
            <Button type="primary" htmlType="submit" loading={loading}>
              Save
            </Button>
          )}
        </div>
      </Form>
    </Drawer>
  )
}
