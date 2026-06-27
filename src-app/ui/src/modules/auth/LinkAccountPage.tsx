import { useEffect, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import {
  Alert,
  Button,
  Card,
  Form,
  FormField,
  PasswordInput,
  Title,
  Paragraph,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { LockOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { BlankLayoutComponent } from '@/modules/layouts/blank'

const linkAccountSchema = z.object({
  password: z.string().min(1, 'Please enter your password'),
})

type LinkFormValues = z.infer<typeof linkAccountSchema>

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
  const form = useForm<LinkFormValues>({
    resolver: zodResolver(linkAccountSchema),
    defaultValues: { password: '' },
  })
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
      await Stores.Auth.linkAccount({ link_token: linkToken, password })
      navigate('/', { replace: true })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to link account')
      setLoading(false)
    }
  }

  return (
    <BlankLayoutComponent>
      <div className="min-h-screen">
        <div className="flex items-center justify-center p-4">
          <Card className="w-full max-w-md">
            <Title level={3}>Link your accounts</Title>
            <Paragraph type="secondary">
              An existing account uses this email. Enter your password to
              confirm ownership and link your social sign-in.
            </Paragraph>
            {error && (
              <Alert
                title={error}
                tone="error"
                onClose={() => setError(null)}
                closeLabel="Close"
                className="mb-4"
              />
            )}
            <Form
              form={form}
              layout="vertical"
              onSubmit={onFinish}
            >
              <FormField
                name="password"
                label="Password"
              >
                <PasswordInput
                  prefix={<LockOutlined />}
                  placeholder="Your existing password"
                  autoComplete="current-password"
                  disabled={!linkToken}
                  showLabel="Show password"
                  hideLabel="Hide password"
                />
              </FormField>
              <Button
                block
                loading={loading}
                disabled={!linkToken}
                type="submit"
              >
                Link and sign in
              </Button>
              <div className="text-center">
                <a href="/auth">Cancel</a>
              </div>
            </Form>
          </Card>
        </div>
      </div>
    </BlankLayoutComponent>
  )
}
