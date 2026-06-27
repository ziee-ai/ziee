import { useState } from 'react'
import { Layout, Title } from '@/components/ui'
import { Stores } from '@/core/stores'
import { LoginForm } from '@/modules/auth/LoginForm'
import { RegisterForm } from '@/modules/auth/RegisterForm'
import { BlankLayoutComponent } from '@/modules/layouts/blank'

const { Content } = Layout

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
    <BlankLayoutComponent>
      <Layout className="min-h-screen">
        <Content className="flex items-center justify-center p-4">
          <div className="w-full max-w-md">
            <div className="text-center mb-8">
              <Title level={2}>Welcome</Title>
            </div>

            {mode === 'login' && (
              <LoginForm onSwitchToRegister={handleSwitchToRegister} />
            )}

            {mode === 'register' && (
              <RegisterForm onSwitchToLogin={() => { Stores.Auth.clearAuthenticationError(); setMode('login') }} />
            )}
          </div>
        </Content>
      </Layout>
    </BlankLayoutComponent>
  )
}
