import { Alert, Button, Card, Form, Input, Typography } from 'antd'
import { LockOutlined, UserOutlined } from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { LoginRequest } from '@/api-client/types'
import { ProviderButtons } from './ProviderButtons'

const { Text } = Typography

interface LoginFormProps {
  onSwitchToRegister?: () => void
}

export const LoginForm: React.FC<LoginFormProps> = ({ onSwitchToRegister }) => {
  const [form] = Form.useForm()
  const { isLoading, error } = Stores.Auth
  const navigate = useNavigate()

  const onFinish = async (values: LoginRequest) => {
    try {
      Stores.Auth.clearAuthenticationError()
      await Stores.Auth.authenticateUser(values)
      // Redirect to home page after successful login
      navigate('/', { replace: true })
    } catch (error) {
      // Error is handled by the store
      console.error('Login failed:', error)
    }
  }

  return (
    <Card className="w-full max-w-md mx-auto">
      {error && (
        <div className="py-4">
          <Alert
            title={error}
            type="error"
            showIcon
            closable={{ closeIcon: true, onClose: Stores.Auth.clearAuthenticationError }}
          />
        </div>
      )}

      <Form
        form={form}
        name="login"
        onFinish={onFinish}
        layout="vertical"
        size="large"
        autoComplete="off"
      >
        <Form.Item
          label="Username or Email"
          name="username"
          rules={[
            { required: true, message: 'Please input your username or email!' },
          ]}
        >
          <Input
            prefix={<UserOutlined />}
            placeholder="Enter your username or email"
            autoComplete="username"
          />
        </Form.Item>

        <Form.Item
          label="Password"
          name="password"
          rules={[{ required: true, message: 'Please input your password!' }]}
        >
          <Input.Password
            prefix={<LockOutlined />}
            placeholder="Enter your password"
            autoComplete="current-password"
          />
        </Form.Item>

        <Form.Item>
          <Button
            type="primary"
            htmlType="submit"
            loading={isLoading}
            className="w-full"
          >
            Sign In
          </Button>
        </Form.Item>

        {onSwitchToRegister && (
          <div className="text-center">
            <Text type="secondary">
              Don't have an account?{' '}
              <Button type="link" onClick={onSwitchToRegister} className="p-0">
                Sign Up
              </Button>
            </Text>
          </div>
        )}
      </Form>

      <ProviderButtons />
    </Card>
  )
}
