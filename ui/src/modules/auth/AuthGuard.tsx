import { Layout, Spin } from 'antd'
import { useEffect } from 'react'
import { initAuth } from './store'
import { Stores } from '@/core/stores'
import { AuthPage } from './AuthPage'

const { Content } = Layout

interface AuthGuardProps {
  children: React.ReactNode
}

export const AuthGuard: React.FC<AuthGuardProps> = ({ children }) => {
  const { isAuthenticated, isLoading } = Stores.Auth

  useEffect(() => {
    initAuth()
  }, [])

  // Show loading spinner while checking auth status
  if (isLoading) {
    return (
      <Layout className="min-h-screen">
        <Content className="flex items-center justify-center">
          <Spin size="large" />
        </Content>
      </Layout>
    )
  }

  // Show authentication page if not authenticated
  if (!isAuthenticated) {
    return <AuthPage />
  }

  // Show the protected content
  return <>{children}</>
}
