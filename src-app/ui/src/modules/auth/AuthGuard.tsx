import { Layout, Spin } from 'antd'
import { useEffect } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { AuthPage } from '@/modules/auth/AuthPage'
import type { OnboardingSlot } from '@/modules/onboarding-screen/types/OnboardingSlot'

const { Content } = Layout

interface AuthGuardProps {
  children: React.ReactNode
}

export const AuthGuard: React.FC<AuthGuardProps> = ({ children }) => {
  const { isAuthenticated, isInitializing, user } = Stores.Auth
  const { needsSetup } = Stores.App
  const guides = (Stores.ModuleSystem.slots.get('onboarding') as OnboardingSlot[]) || []
  const navigate = useNavigate()
  const location = useLocation()

  useEffect(() => {
    // Initialize auth (setup status already checked by app module)
    Stores.Auth.initAuth()
  }, [])

  useEffect(() => {
    // Redirect to setup if needed
    if (needsSetup === true && location.pathname !== '/setup') {
      navigate('/setup', { replace: true })
    }
  }, [needsSetup, navigate, location.pathname])

  // Show loading spinner while checking auth status
  if (isInitializing || needsSetup === null) {
    return (
      <Layout className="min-h-screen">
        <Content className="flex items-center justify-center">
          <Spin size="large" />
        </Content>
      </Layout>
    )
  }

  // Redirect to setup if needed
  if (needsSetup) {
    navigate('/setup', { replace: true })
    return null
  }

  // Show authentication page if not authenticated
  if (!isAuthenticated) {
    return <AuthPage />
  }

  // Redirect to incomplete guide if user hasn't finished it
  const isOnGuideRoute = location.pathname.startsWith('/onboarding-screen')
  if (user && !isOnGuideRoute) {
    const firstIncomplete = guides.find(g => !user.completed_onboarding_ids.includes(g.id))
    if (firstIncomplete) {
      navigate(`/onboarding-screen?id=${firstIncomplete.id}`, { replace: true })
      return null
    }
  }

  // Show the protected content
  return <>{children}</>
}
