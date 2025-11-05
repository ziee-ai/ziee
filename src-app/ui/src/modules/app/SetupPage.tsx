import React from 'react'
import { Card, Form, Input, Button, Typography, Alert } from 'antd'
import { useNavigate } from 'react-router-dom'
import { setupAdmin, clearSetupError } from './store'
import { authenticateUser } from '../auth/store'
import { Stores } from '@/core'

const { Title, Paragraph } = Typography

export default function SetupPage() {
  const { needsSetup, isSettingUpAdmin, setupError } = Stores.App
  const navigate = useNavigate()
  const [form] = Form.useForm()

  console.log({ needsSetup })

  // Redirect to homepage if setup is not needed
  React.useEffect(() => {
    console.log('useEffect needsSetup:', needsSetup)
    if (needsSetup === false) {
      navigate('/', { replace: true })
    }
  }, [needsSetup, navigate])

  const onFinish = async (values: any) => {
    try {
      await setupAdmin({
        username: values.username,
        email: values.email,
        password: values.password,
        display_name: values.display_name,
      })

      // Use the login credentials to authenticate
      await authenticateUser({
        username: values.username,
        password: values.password,
      })

      // Redirect to dashboard
      navigate('/', { replace: true })
    } catch (err) {
      // Error is already handled in the store
      console.error('Setup failed:', err)
    }
  }

  const validatePassword = (_: any, value: string) => {
    if (!value) {
      return Promise.reject(new Error('Password is required'))
    }
    if (value.length < 8) {
      return Promise.reject(new Error('Password must be at least 8 characters'))
    }

    return Promise.resolve()
  }

  return (
    <div className="min-h-screen flex items-center justify-center  p-4">
      <Card className="w-full max-w-md">
        <div className="mb-6">
          <Title level={2}>Welcome to Ziee Chat</Title>
          <Paragraph>
            No administrator account exists. Let's create your first admin
            account to get started.
          </Paragraph>
        </div>

        {setupError && (
          <Alert
            type="error"
            message={setupError}
            className="mb-4"
            closable
            onClose={clearSetupError}
          />
        )}

        <Form form={form} layout="vertical" onFinish={onFinish}>
          <Form.Item
            label="Username"
            name="username"
            rules={[
              { required: true, message: 'Username is required' },
              { min: 3, message: 'Username must be at least 3 characters' },
              {
                max: 100,
                message: 'Username must be less than 100 characters',
              },
              {
                pattern: /^[a-zA-Z0-9_-]+$/,
                message:
                  'Username can only contain letters, numbers, hyphens, and underscores',
              },
            ]}
          >
            <Input placeholder="admin" autoComplete="username" />
          </Form.Item>

          <Form.Item
            label="Email"
            name="email"
            rules={[
              { required: true, message: 'Email is required' },
              { type: 'email', message: 'Invalid email format' },
              { max: 255, message: 'Email must be less than 255 characters' },
            ]}
          >
            <Input
              placeholder="admin@example.com"
              type="email"
              autoComplete="email"
            />
          </Form.Item>

          <Form.Item
            label="Password"
            name="password"
            rules={[{ validator: validatePassword }]}
            help="Must be at least 8 characters"
          >
            <Input.Password
              placeholder="Enter a strong password"
              autoComplete="new-password"
            />
          </Form.Item>

          <Form.Item
            label="Confirm Password"
            name="confirm_password"
            dependencies={['password']}
            rules={[
              { required: true, message: 'Please confirm your password' },
              ({ getFieldValue }) => ({
                validator(_, value) {
                  if (!value || getFieldValue('password') === value) {
                    return Promise.resolve()
                  }
                  return Promise.reject(new Error('Passwords do not match'))
                },
              }),
            ]}
          >
            <Input.Password
              placeholder="Confirm your password"
              autoComplete="new-password"
            />
          </Form.Item>

          <Form.Item label="Display Name (Optional)" name="display_name">
            <Input placeholder="System Administrator" />
          </Form.Item>

          <Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              block
              loading={isSettingUpAdmin}
              size="large"
            >
              Create Admin Account
            </Button>
          </Form.Item>
        </Form>
      </Card>
    </div>
  )
}
