import { App, Button, Flex, Form, Input } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

export function ResetPasswordDrawer() {
  const { message } = App.useApp()
  const { isOpen, user } = Stores.ResetPasswordDrawer
  const [passwordForm] = Form.useForm()
  const canReset = usePermission(Permissions.UsersResetPassword)

  const handleResetPassword = async (values: any) => {
    if (!user) return

    try {
      await Stores.Users.resetUserPassword(user.id, values.new_password)

      message.success('Password reset successfully')
      Stores.ResetPasswordDrawer.closeResetPasswordDrawer()
      passwordForm.resetFields()
    } catch (error) {
      console.error('Failed to reset password:', error)
      // Error is handled by the store
    }
  }

  return (
    <Drawer
      title="Reset Password"
      open={isOpen}
      onClose={() => {
        Stores.ResetPasswordDrawer.closeResetPasswordDrawer()
        passwordForm.resetFields()
      }}
      footer={null}
      mask={{ closable: false }}
    >
      <Form
        form={passwordForm}
        layout="vertical"
        onFinish={handleResetPassword}
        disabled={!canReset}
      >
        <Form.Item
          name="new_password"
          label="New Password"
          rules={[
            { required: true, message: 'Please enter new password' },
            { min: 6, message: 'Password must be at least 6 characters' },
          ]}
        >
          <Input.Password placeholder="Enter new password" />
        </Form.Item>
        <Form.Item
          name="confirm_password"
          label="Confirm Password"
          dependencies={['new_password']}
          rules={[
            { required: true, message: 'Please confirm password' },
            ({ getFieldValue }) => ({
              validator(_, value) {
                if (!value || getFieldValue('new_password') === value) {
                  return Promise.resolve()
                }
                return Promise.reject('Passwords do not match')
              },
            }),
          ]}
        >
          <Input.Password placeholder="Confirm new password" />
        </Form.Item>
        <Form.Item className="mb-0">
          <Flex className="justify-end gap-2">
            <Button
              onClick={() => {
                Stores.ResetPasswordDrawer.closeResetPasswordDrawer()
                passwordForm.resetFields()
              }}
            >
              {canReset ? 'Cancel' : 'Close'}
            </Button>
            {canReset && (
              <Button type="primary" htmlType="submit">
                Reset
              </Button>
            )}
          </Flex>
        </Form.Item>
      </Form>
    </Drawer>
  )
}
