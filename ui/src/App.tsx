import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { useRouterStore } from './core'
import { AuthGuard } from './modules/auth'
import AppLayout from './components/Layout/AppLayout'
import { ThemeProvider } from './components/ThemeProvider'
import { loadModules } from './modules/loader'

// Load all modules before rendering
loadModules()

function App() {
  const { routes } = useRouterStore()

  return (
    <ThemeProvider>
      <BrowserRouter>
        <Routes>
          {/* Dynamically render routes from modules */}
          {routes.map((route, index) => {
            const element = route.requiresAuth ? (
              <AuthGuard>
                {route.layout === 'default' ? (
                  <AppLayout>{route.element}</AppLayout>
                ) : (
                  route.element
                )}
              </AuthGuard>
            ) : (
              route.element
            )

            return (
              <Route
                key={`${route.path}-${index}`}
                path={route.path}
                element={element}
                index={route.index}
              />
            )
          })}

          {/* Fallback route */}
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </BrowserRouter>
    </ThemeProvider>
  )
}

export default App
