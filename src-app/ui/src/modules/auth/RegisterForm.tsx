import {
  Alert,
  Button,
  Card,
  Form,
  FormField,
  Input,
  PasswordInput,
  Title,
  Text,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { LockOutlined, MailOutlined, UserOutlined } from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { CreateUserRequest } from '@/api-client/types'

interface RegisterFormProps {
  onSwitchToLogin?: () => void
}

interface RegisterFormValues {
  username: string
  email: string
  password: string
  confirmPassword: string
}

const schema = z
  .object({
    username: z
      .string()
      .min(1, 'Please input your username!')
      .min(3, 'Username must be at least 3 characters long!'),
    email: z
      .string()
      .min(1, 'Please input your email!')
      .email('Please enter a valid email address!'),
    password: z
      .string()
      .min(1, 'Please input your password!')
      .min(6, 'Password must be at least 6 characters long!'),
    confirmPassword: z.string().min(1, 'Please confirm your password!'),
  })
  .refine(data => data.password === data.confirmPassword, {
    message: 'Passwords do not match!',
    path: ['confirmPassword'],
  })

export const RegisterForm: React.FC<RegisterFormProps> = ({
  onSwitchToLogin,
}) => {
  const form = useForm<RegisterFormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      username: '',
      email: '',
      password: '',
      confirmPassword: '',
    },
  })
  const { isLoading, error } = Stores.Auth
  const navigate = useNavigate()

  const onSubmit = async (values: RegisterFormValues) => {
    try {
      Stores.Auth.clearAuthenticationError()
      await Stores.Auth.registerNewUser({
        username: values.username,
        email: values.email,
        password: values.password,
      } as CreateUserRequest)
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
          title={error}
          tone="error"
          onClose={Stores.Auth.clearAuthenticationError}
          closeLabel="Close"
          className="mb-4"
        />
      )}

      <Form
        form={form}
        name="register"
        onSubmit={onSubmit}
        layout="vertical"
        size="lg"
      >
        <FormField label="Username" name="username">
          <Input
            prefix={<UserOutlined />}
            placeholder="Enter your username"
            autoComplete="username"
          />
        </FormField>

        <FormField label="Email" name="email">
          <Input
            prefix={<MailOutlined />}
            placeholder="Enter your email address"
            autoComplete="email"
          />
        </FormField>

        <FormField label="Password" name="password">
          <PasswordInput
            prefix={<LockOutlined />}
            placeholder="Enter your password"
            autoComplete="new-password"
            showLabel="Show password"
            hideLabel="Hide password"
          />
        </FormField>

        <FormField label="Confirm Password" name="confirmPassword">
          <PasswordInput
            prefix={<LockOutlined />}
            placeholder="Confirm your password"
            autoComplete="new-password"
            showLabel="Show password"
            hideLabel="Hide password"
          />
        </FormField>

        <Button type="submit" loading={isLoading} className="w-full">
          Sign Up
        </Button>

        {onSwitchToLogin && (
          <div className="text-center">
            <Text type="secondary">
              Already have an account?{' '}
              <Button variant="link" onClick={onSwitchToLogin} className="p-0">
                Sign In
              </Button>
            </Text>
          </div>
        )}
      </Form>
    </Card>
  )
}
