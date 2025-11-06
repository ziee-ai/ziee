import { Alert, Button, Card, Form, Input, Typography } from 'antd'
import { LockOutlined, MailOutlined, UserOutlined } from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { CreateUserRequest } from '../../api-client/types'

const { Title, Text } = Typography

interface RegisterFormProps {
  onSwitchToLogin?: () => void
}

export const RegisterForm: React.FC<RegisterFormProps> = ({
  onSwitchToLogin,
}) => {
  const [form] = Form.useForm()
  const { isLoading, error } = Stores.Auth
  const navigate = useNavigate()

  const onFinish = async (values: CreateUserRequest) => {
    try {
      Stores.Auth.clearAuthenticationError()
      await Stores.Auth.registerNewUser(values)
      // Redirect to home page after successful registration
      navigate('/', { replace: true })
    } catch (error) {
      // Error is handled by the store
      console.error('Registration failed:', error)
    }
  }

  return (
    <Card className="w-full max-w-md mx-auto">
      <div className="text-center mb-6">
        <Title level={3}>Create Account</Title>
      </div>

      {error && (
        <Alert
          message={error}
          type="error"
          showIcon
          closable
          onClose={Stores.Auth.clearAuthenticationError}
          className="mb-4"
        />
      )}

      <Form
        form={form}
        name="register"
        onFinish={onFinish}
        layout="vertical"
        size="large"
        autoComplete="off"
      >
        <Form.Item
          label="Username"
          name="username"
          rules={[
            { required: true, message: 'Please input your username!' },
            { min: 3, message: 'Username must be at least 3 characters long!' },
          ]}
        >
          <Input
            prefix={<UserOutlined />}
            placeholder="Enter your username"
            autoComplete="username"
          />
        </Form.Item>

        <Form.Item
          label="Email"
          name="email"
          rules={[
            { required: true, message: 'Please input your email!' },
            { type: 'email', message: 'Please enter a valid email address!' },
          ]}
        >
          <Input
            prefix={<MailOutlined />}
            placeholder="Enter your email address"
            autoComplete="email"
          />
        </Form.Item>

        <Form.Item
          label="Password"
          name="password"
          rules={[
            { required: true, message: 'Please input your password!' },
            { min: 6, message: 'Password must be at least 6 characters long!' },
          ]}
        >
          <Input.Password
            prefix={<LockOutlined />}
            placeholder="Enter your password"
            autoComplete="new-password"
          />
        </Form.Item>

        <Form.Item
          label="Confirm Password"
          name="confirmPassword"
          dependencies={['password']}
          rules={[
            { required: true, message: 'Please confirm your password!' },
            ({ getFieldValue }) => ({
              validator(_, value) {
                if (!value || getFieldValue('password') === value) {
                  return Promise.resolve()
                }
                return Promise.reject(new Error('Passwords do not match!'))
              },
            }),
          ]}
        >
          <Input.Password
            prefix={<LockOutlined />}
            placeholder="Confirm your password"
            autoComplete="new-password"
          />
        </Form.Item>

        <Form.Item>
          <Button
            type="primary"
            htmlType="submit"
            loading={isLoading}
            className="w-full"
          >
            Sign Up
          </Button>
        </Form.Item>

        {onSwitchToLogin && (
          <div className="text-center">
            <Text type="secondary">
              Already have an account?{' '}
              <Button type="link" onClick={onSwitchToLogin} className="p-0">
                Sign In
              </Button>
            </Text>
          </div>
        )}
      </Form>
    </Card>
  )
}
