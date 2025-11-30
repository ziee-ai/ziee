import { App, Button, Flex, Form, Input } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import type { CreateUserRequest } from '@/api-client/types'
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

export function CreateUserDrawer() {
  const { message } = App.useApp()
  const { isOpen } = Stores.CreateUserDrawer
  const { creating: creatingUser } = Stores.Users
  const [createForm] = Form.useForm()

  const handleCreateUser = async (values: any) => {
    try {
      const userData: CreateUserRequest = {
        username: values.username,
        email: values.email,
        password: values.password,
        display_name: values.display_name,
        permissions: values.permissions
          ? JSON.parse(values.permissions)
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
      maskClosable={false}
    >
      <Form name="create-user" form={createForm} layout="vertical" onFinish={handleCreateUser}>
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
        <Form.Item
          name="permissions"
          label="Permissions (JSON Array)"
          rules={[{ validator: validatePermissions }]}
        >
          <TextArea rows={6} placeholder='["users::read", "users::edit"]' />
        </Form.Item>
        <Form.Item className="mb-0">
          <Flex className="gap-2">
            <Button type="primary" htmlType="submit" loading={creatingUser}>
              Create User
            </Button>
            <Button
              onClick={() => {
                Stores.CreateUserDrawer.closeCreateUserDrawer()
                createForm.resetFields()
              }}
              disabled={creatingUser}
            >
              Cancel
            </Button>
          </Flex>
        </Form.Item>
      </Form>
    </Drawer>
  )
}
