import { useEffect } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { AuthPage } from '@/modules/auth/AuthPage'
import { Loading } from '@/core/components/Loading'

interface AuthGuardProps {
  children: React.ReactNode
}

export const AuthGuard: React.FC<AuthGuardProps> = ({ children }) => {
  const { isAuthenticated, isInitializing } = Stores.Auth
  const { needsSetup } = Stores.App
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
    return <Loading fullscreen />
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

  // Show the protected content. Post-auth redirects (onboarding,
  // etc.) are owned by the contributing module — see the
  // `routerEffects` slot consumed by RouterComponent.
  return <>{children}</>
}
