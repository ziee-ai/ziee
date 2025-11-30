import { App, Button, Flex, Form, Input, Switch } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import type { UpdateUserRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { useEffect } from 'react'

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
    const invalidPermissions = parsed.filter(
      perm => !validPermissions.includes(perm),
    )

    if (invalidPermissions.length > 0) {
      return Promise.reject(
        `Invalid permissions: ${invalidPermissions.join(', ')}`,
      )
    }

    return Promise.resolve()
  } catch {
    return Promise.reject('Invalid JSON format')
  }
}

export function EditUserDrawer() {
  const { message } = App.useApp()
  const { isOpen, editingUser } = Stores.EditUserDrawer
  const [editForm] = Form.useForm()

  // Update form when editingUser changes
  useEffect(() => {
    if (editingUser) {
      editForm.setFieldsValue({
        username: editingUser.username,
        email: editingUser.email,
        is_active: editingUser.is_active,
        permissions:
          editingUser.permissions?.length > 0
            ? JSON.stringify(editingUser.permissions, null, 2)
            : '',
      })
    }
  }, [editingUser, editForm])

  const handleEditUser = async (values: any) => {
    if (!editingUser) return

    try {
      const updateData: UpdateUserRequest = {
        username: values.username,
        email: values.email,
        is_active: values.is_active,
        permissions: values.permissions
          ? JSON.parse(values.permissions)
          : undefined,
      }

      await Stores.Users.updateUser(editingUser.id, updateData)

      message.success('User updated successfully')
      Stores.EditUserDrawer.closeEditUserDrawer()
      editForm.resetFields()
    } catch (error) {
      console.error('Failed to update user:', error)
      // Error is handled by the store
    }
  }

  return (
    <Drawer
      title="Edit User"
      open={isOpen}
      onClose={() => {
        Stores.EditUserDrawer.closeEditUserDrawer()
        editForm.resetFields()
      }}
      footer={null}
      size={600}
      maskClosable={false}
    >
      <Form
        name="edit-user-form"
        form={editForm}
        layout="vertical"
        onFinish={handleEditUser}
      >
        <Form.Item
          name="username"
          label="Username"
          rules={[{ required: true, message: 'Please enter username' }]}
        >
          <Input placeholder="Enter username" />
        </Form.Item>
        <Form.Item
          name="email"
          label="Email"
          rules={[
            {
              required: true,
              type: 'email',
              message: 'Please enter valid email',
            },
          ]}
        >
          <Input placeholder="Enter email" />
        </Form.Item>
        <Form.Item name="is_active" label="Active" valuePropName="checked">
          <Switch />
        </Form.Item>
        <Form.Item
          name="permissions"
          label="Permissions (JSON Array)"
          rules={[{ validator: validatePermissions }]}
        >
          <TextArea rows={6} placeholder='["users::read", "users::edit"]' />
        </Form.Item>
        <Form.Item className="mb-0">
          <Flex className="gap-2">
            <Button type="primary" htmlType="submit">
              Update User
            </Button>
            <Button
              onClick={() => {
                Stores.EditUserDrawer.closeEditUserDrawer()
                editForm.resetFields()
              }}
            >
              Cancel
            </Button>
          </Flex>
        </Form.Item>
      </Form>
    </Drawer>
  )
}
