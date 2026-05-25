import { App, Button, Flex, Form, Input, Switch } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import type { UpdateUserRequest } from '@/api-client/types'
import { useEffect } from 'react'

export function EditUserDrawer() {
  const { message } = App.useApp()
  const { isOpen, editingUser } = Stores.EditUserDrawer
  const [editForm] = Form.useForm()

  // Update form when editingUser changes
  useEffect(() => {
    if (editingUser) {
      editForm.setFieldsValue({
        username: editingUser.username,
        display_name: editingUser.display_name ?? '',
        is_active: editingUser.is_active,
      })
    }
  }, [editingUser, editForm])

  const handleEditUser = async (values: any) => {
    if (!editingUser) return

    try {
      // NOTE: `email` and `permissions` are intentionally NOT included.
      // The backend's UpdateUserRequest dropped both fields:
      //   - `email`: changing email without confirmation enables OAuth
      //     account takeover (03-user F-03 closure).
      //   - `permissions`: lets any users::edit holder escalate to
      //     wildcard '*' (03-user F-01 closure). Permissions are now
      //     managed via group assignment only.
      const updateData: UpdateUserRequest = {
        username: values.username,
        display_name: values.display_name || null,
        is_active: values.is_active,
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
        <Form.Item name="display_name" label="Display Name">
          <Input placeholder="Enter display name (optional)" />
        </Form.Item>
        <Form.Item name="is_active" label="Active" valuePropName="checked">
          <Switch />
        </Form.Item>
        {/*
         * Email + Permissions removed from this form per security work:
         * - Email: changing without confirmation enables OAuth takeover
         *   (03-user F-03). Add a verification-token flow before
         *   restoring.
         * - Permissions: editing them here let a sub-admin grant
         *   themselves wildcard '*' (03-user F-01). Use group
         *   assignment (POST /api/groups/{id}/users) instead.
         */}
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
