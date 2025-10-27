import { useState } from 'react'
import { Layout, Typography } from 'antd'
import { Stores } from '@/core/stores'
import { LoginForm } from './LoginForm'
import { RegisterForm } from './RegisterForm'

const { Content } = Layout
const { Title } = Typography

type AuthMode = 'login' | 'register'

export const AuthPage: React.FC = () => {
  const [mode, setMode] = useState<AuthMode>('login')
  const { isAuthenticated } = Stores.Auth

  const handleSwitchToRegister = () => {
    setMode('register')
  }

  // Don't render anything if already authenticated
  if (isAuthenticated) {
    return null
  }

  return (
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
            <RegisterForm onSwitchToLogin={() => setMode('login')} />
          )}
        </div>
      </Content>
    </Layout>
  )
}
