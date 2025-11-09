import { App, Button, Form, Input, Switch } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import type { UpdateGroupRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/types'

const { TextArea } = Input

// Helper function to validate permissions
const validatePermissions = (_: any, value: string) => {
  if (!value) return Promise.resolve()

  try {
    const parsed = JSON.parse(value)
    if (!Array.isArray(parsed)) {
      return Promise.reject('Must be an array')
    }

    // Check if all values are valid permissions
    const validPermissions = Object.values(Permissions)
    const invalidPermissions = parsed.filter(perm => !validPermissions.includes(perm))

    if (invalidPermissions.length > 0) {
      return Promise.reject(`Invalid permissions: ${invalidPermissions.join(', ')}`)
    }

    return Promise.resolve()
  } catch {
    return Promise.reject('Invalid JSON format')
  }
}

export function EditUserGroupDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  const { isOpen: open, editingGroup: group } = Stores.EditUserGroupDrawer

  // Load group data when it changes
  useEffect(() => {
    if (group && open) {
      form.setFieldsValue({
        name: group.name,
        description: group.description,
        permissions: JSON.stringify(group.permissions, null, 2),
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

      let permissions: string[] = []
      try {
        permissions = JSON.parse(values.permissions || '[]')
        if (!Array.isArray(permissions)) {
          throw new Error('Permissions must be an array')
        }
      } catch (_error) {
        message.error(
          'Invalid permissions format. Please enter a valid JSON array.',
        )
        return
      }

      const updateData: UpdateGroupRequest = {
        name: values.name,
        description: values.description,
        permissions,
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
      width={600}
      maskClosable={false}
    >
      <Form name="edit-user-group-form" form={form} layout="vertical" onFinish={handleSubmit}>
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

        <Form.Item
          name="permissions"
          label="Permissions (JSON Array)"
          rules={[{ validator: validatePermissions }]}
        >
          <TextArea
            placeholder='["users::read", "users::edit"]'
            rows={6}
          />
        </Form.Item>

        <Form.Item name="is_active" label="Active" valuePropName="checked">
          <Switch aria-label="Set group as active or inactive" />
        </Form.Item>

        <div className="flex justify-end gap-3 pt-4">
          <Button onClick={handleClose} disabled={loading}>
            Cancel
          </Button>
          <Button
            type="primary"
            htmlType="submit"
            loading={loading}
          >
            Update Group
          </Button>
        </div>
      </Form>
    </Drawer>
  )
}
