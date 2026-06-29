import React from 'react'
import { z } from 'zod'
import { Card, Form, FormField, useForm, zodResolver, Input, Button, Alert, Title, Paragraph } from '@/components/ui'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core'

const setupSchema = z
  .object({
    username: z
      .string()
      .min(1, 'Username is required')
      .min(3, 'Username must be at least 3 characters')
      .max(100, 'Username must be less than 100 characters')
      .regex(
        /^[a-zA-Z0-9_-]+$/,
        'Username can only contain letters, numbers, hyphens, and underscores',
      ),
    email: z
      .string()
      .min(1, 'Email is required')
      .email('Invalid email format')
      .max(255, 'Email must be less than 255 characters'),
    password: z
      .string()
      .min(1, 'Password is required')
      .min(8, 'Password must be at least 8 characters'),
    confirm_password: z.string().min(1, 'Please confirm your password'),
    display_name: z.string().optional(),
  })
  .refine((data) => data.password === data.confirm_password, {
    message: 'Passwords do not match',
    path: ['confirm_password'],
  })

type SetupValues = z.infer<typeof setupSchema>

export default function SetupPage() {
  const { needsSetup, isSettingUpAdmin, setupError } = Stores.App
  const navigate = useNavigate()
  const form = useForm<SetupValues>({
    resolver: zodResolver(setupSchema),
    defaultValues: {
      username: '',
      email: '',
      password: '',
      confirm_password: '',
      display_name: '',
    },
  })

  // Redirect away if setup is already done (admin exists). Two paths
  // benefit:
  //   1. Cross-tab: tab A still on /setup when tab B completes setup.
  //   2. Direct nav: tests / users hitting /setup when admin already
  //      exists (e.g. an API-only setup happened before the page load).
  //
  // The earlier race (this navigate firing mid-onFinish and aborting
  // the in-flight /me from authenticateUser) is now defused upstream:
  // Auth.store's catch keeps the token across a TypeError/Failed-to-
  // fetch (abort) so the next mount can retry. Re-enabled here.
  React.useEffect(() => {
    if (needsSetup === false) {
      navigate('/', { replace: true })
    }
  }, [needsSetup, navigate])

  const onSubmit = async (values: SetupValues) => {
    try {
      await Stores.App.setupAdmin({
        username: values.username,
        email: values.email,
        password: values.password,
        display_name: values.display_name,
      })

      // Use the login credentials to authenticate
      await Stores.Auth.authenticateUser({
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

  return (
    <div className="min-h-screen flex items-center justify-center  p-4">
      <Card className="w-full max-w-md" data-testid="app-setup-card">
        <div className="mb-6">
          <Title level={2}>Welcome to Ziee</Title>
          <Paragraph>
            No administrator account exists. Let's create your first admin
            account to get started.
          </Paragraph>
        </div>

        {setupError && (
          <Alert
            tone="error"
            data-testid="app-setup-error-alert"
            title={setupError}
            className="mb-4"
            onClose={Stores.App.clearSetupError}
            closeLabel="Close"
          />
        )}

        <Form
          name="setup-form"
          data-testid="app-setup-form"
          form={form}
          layout="vertical"
          onSubmit={onSubmit}
        >
          <FormField
            label="Username"
            name="username"
            required
          >
            <Input data-testid="app-setup-username-input" placeholder="admin" autoComplete="username" autoFocus />
          </FormField>

          <FormField
            label="Email"
            name="email"
            required
          >
            <Input
              data-testid="app-setup-email-input"
              placeholder="admin@example.com"
              type="email"
              autoComplete="email"
            />
          </FormField>

          <FormField
            label="Password"
            name="password"
            required
            description="Must be at least 8 characters"
          >
            <Input
              data-testid="app-setup-password-input"
              type="password"
              placeholder="Enter a strong password"
              autoComplete="new-password"
            />
          </FormField>

          <FormField
            label="Confirm Password"
            name="confirm_password"
            required
          >
            <Input
              data-testid="app-setup-confirm-password-input"
              type="password"
              placeholder="Confirm your password"
              autoComplete="new-password"
            />
          </FormField>

          <FormField label="Display Name (Optional)" name="display_name">
            <Input data-testid="app-setup-display-name-input" placeholder="System Administrator" />
          </FormField>

          <Button
            type="submit"
            data-testid="app-setup-submit-button"
            block
            loading={isSettingUpAdmin}
            size="lg"
            className="w-full"
          >
            Create Admin Account
          </Button>
        </Form>
      </Card>
    </div>
  )
}
