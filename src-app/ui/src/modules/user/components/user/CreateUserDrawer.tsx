import { App, Button, Flex, Form, Input } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import type { CreateUserRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { PermissionsField } from '@/modules/user/components/PermissionsField.tsx'

export function CreateUserDrawer() {
  const { message } = App.useApp()
  const { isOpen } = Stores.CreateUserDrawer
  const { creating: creatingUser } = Stores.Users
  const [createForm] = Form.useForm()
  const canCreate = usePermission(Permissions.UsersCreate)

  const handleCreateUser = async (values: any) => {
    try {
      const userData: CreateUserRequest = {
        username: values.username,
        email: values.email,
        password: values.password,
        display_name: values.display_name,
        permissions: values.permissions?.length
          ? values.permissions
          : undefined,
      }

      await Stores.Users.createUser(userData)

      message.success('User created successfully')
      Stores.CreateUserDrawer.closeCreateUserDrawer()
      createForm.resetFields()
    } catch (error) {
      console.error('Failed to create user:', error)
      // Error is handled by the store
    }
  }

  return (
    <Drawer
      title="Create User"
      open={isOpen}
      onClose={() => {
        Stores.CreateUserDrawer.closeCreateUserDrawer()
        createForm.resetFields()
      }}
      footer={null}
      size={600}
      mask={{ closable: false }}
    >
      <Form
        name="create-user"
        form={createForm}
        layout="vertical"
        onFinish={handleCreateUser}
        disabled={!canCreate}
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
        <Form.Item
          name="password"
          label="Password"
          rules={[
            { required: true, message: 'Please enter password' },
            { min: 6, message: 'Password must be at least 6 characters' },
          ]}
        >
          <Input.Password placeholder="Enter password" />
        </Form.Item>
        <Form.Item name="display_name" label="Display Name">
          <Input placeholder="Enter display name (optional)" />
        </Form.Item>
        <Form.Item name="permissions" label="Permissions">
          <PermissionsField disabled={!canCreate} />
        </Form.Item>
        <Form.Item className="mb-0">
          <Flex className="justify-end gap-2">
            <Button
              onClick={() => {
                Stores.CreateUserDrawer.closeCreateUserDrawer()
                createForm.resetFields()
              }}
              disabled={creatingUser}
            >
              {canCreate ? 'Cancel' : 'Close'}
            </Button>
            {canCreate && (
              <Button type="primary" htmlType="submit" loading={creatingUser}>
                Create
              </Button>
            )}
          </Flex>
        </Form.Item>
      </Form>
    </Drawer>
  )
}
