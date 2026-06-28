import { z } from 'zod'
import {
  Alert,
  Button,
  Card,
  Form,
  FormField,
  Input,
  PasswordInput,
  Text,
  useForm,
  zodResolver,
} from '@/components/ui'
import { LockOutlined, UserOutlined } from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { LoginRequest } from '@/api-client/types'
import { ProviderButtons } from './ProviderButtons'

interface LoginFormProps {
  onSwitchToRegister?: () => void
}

const loginSchema = z.object({
  username: z.string().min(1, 'Please input your username or email!'),
  password: z.string().min(1, 'Please input your password!'),
})

export const LoginForm: React.FC<LoginFormProps> = ({ onSwitchToRegister }) => {
  const form = useForm<LoginRequest>({
    resolver: zodResolver(loginSchema),
    defaultValues: { username: '', password: '' },
  })
  const { isLoading, error } = Stores.Auth
  const navigate = useNavigate()

  const onSubmit = async (values: LoginRequest) => {
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
            tone="error"
            onClose={Stores.Auth.clearAuthenticationError}
            closeLabel="Close"
          />
        </div>
      )}

      <Form form={form} name="login" onSubmit={onSubmit} layout="vertical" size="lg">
        <FormField label="Username or Email" name="username">
          <Input
            prefix={<UserOutlined />}
            placeholder="Enter your username or email"
            autoComplete="username"
          />
        </FormField>

        <FormField label="Password" name="password">
          <PasswordInput
            prefix={<LockOutlined />}
            placeholder="Enter your password"
            autoComplete="current-password"
            showLabel="Show password"
            hideLabel="Hide password"
          />
        </FormField>

        <Button type="submit" loading={isLoading} className="w-full">
          Sign In
        </Button>

        {onSwitchToRegister && (
          <div className="text-center">
            <Text type="secondary">
              Don't have an account?{' '}
              <Button variant="link" onClick={onSwitchToRegister} className="p-0">
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
