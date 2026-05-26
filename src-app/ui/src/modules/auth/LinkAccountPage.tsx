import { useEffect, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import {
  Alert,
  Button,
  Card,
  Form,
  Input,
  Layout,
  Typography,
} from 'antd'
import { LockOutlined } from '@ant-design/icons'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import { BlankLayoutComponent } from '@/modules/layouts/blank'

const { Content } = Layout
const { Title, Paragraph } = Typography

interface LinkFormValues {
  password: string
}

/**
 * /auth/link-account — First-Broker-Login confirmation page.
 *
 * Reached when a social-login email collides with an existing local
 * account. The backend stored a single-use `link_token` referenced
 * in `?link_token=...`; the user proves ownership by entering their
 * existing local password. On success the backend atomically creates
 * the user_auth_links row and returns a fresh JWT pair (same shape
 * as a normal login response).
 */
export const LinkAccountPage: React.FC = () => {
  const [params] = useSearchParams()
  const navigate = useNavigate()
  const [form] = Form.useForm<LinkFormValues>()
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const linkToken = params.get('link_token') ?? ''

  useEffect(() => {
    if (!linkToken) {
      setError('Missing link token. Return to login and try again.')
    }
  }, [linkToken])

  const onFinish = async ({ password }: LinkFormValues) => {
    if (!linkToken) return
    setError(null)
    setLoading(true)
    try {
      const res = await ApiClient.Auth.linkAccount(
        { link_token: linkToken, password },
        undefined,
      )
      Stores.Auth.setAuthFromAutoLogin({
        user: res.user,
        access_token: res.access_token,
        refresh_token: res.refresh_token,
      })
      await Stores.Auth.initAuth()
      navigate('/', { replace: true })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to link account')
      setLoading(false)
    }
  }

  return (
    <BlankLayoutComponent>
      <Layout className="min-h-screen">
        <Content className="flex items-center justify-center p-4">
          <Card className="w-full max-w-md">
            <Title level={3}>Link your accounts</Title>
            <Paragraph type="secondary">
              An existing account uses this email. Enter your password to
              confirm ownership and link your social sign-in.
            </Paragraph>
            {error && (
              <Alert
                title={error}
                type="error"
                showIcon
                closable={{ onClose: () => setError(null) }}
                className="mb-4"
              />
            )}
            <Form
              form={form}
              layout="vertical"
              size="large"
              onFinish={onFinish}
              autoComplete="off"
            >
              <Form.Item
                label="Password"
                name="password"
                rules={[
                  { required: true, message: 'Please enter your password' },
                ]}
              >
                <Input.Password
                  prefix={<LockOutlined />}
                  placeholder="Your existing password"
                  autoComplete="current-password"
                  disabled={!linkToken}
                />
              </Form.Item>
              <Form.Item>
                <Button
                  type="primary"
                  htmlType="submit"
                  block
                  loading={loading}
                  disabled={!linkToken}
                >
                  Link and sign in
                </Button>
              </Form.Item>
              <div className="text-center">
                <a href="/auth">Cancel</a>
              </div>
            </Form>
          </Card>
        </Content>
      </Layout>
    </BlankLayoutComponent>
  )
}
