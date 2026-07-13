import { useState } from 'react'
import { Title } from '@/components/ui'
import { Stores } from '@/core/stores'
import { LoginForm } from '@/modules/auth/LoginForm'
import { RegisterForm } from '@/modules/auth/RegisterForm'
import { AuthScreenLayout } from '@/modules/auth/AuthScreenLayout'

type AuthMode = 'login' | 'register'

export const AuthPage: React.FC = () => {
  const [mode, setMode] = useState<AuthMode>('login')
  const { isAuthenticated } = Stores.Auth

  const handleSwitchToRegister = () => {
    Stores.Auth.clearAuthenticationError()
    setMode('register')
  }

  // Don't render anything if already authenticated
  if (isAuthenticated) {
    return null
  }

  return (
    <AuthScreenLayout>
      <div data-testid="auth-page-content" className="w-full">
        <div className="text-center mb-8">
          <Title level={2}>
            {mode === 'login' ? 'Welcome back' : 'Create your account'}
          </Title>
        </div>

        {mode === 'login' && (
          <LoginForm onSwitchToRegister={handleSwitchToRegister} />
        )}

        {mode === 'register' && (
          <RegisterForm
            onSwitchToLogin={() => {
              Stores.Auth.clearAuthenticationError()
              setMode('login')
            }}
          />
        )}
      </div>
    </AuthScreenLayout>
  )
}
