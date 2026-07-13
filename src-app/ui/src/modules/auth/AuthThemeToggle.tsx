import { Sun, Moon } from 'lucide-react'
import { Button } from '@/components/ui'
import { useTheme } from '@/hooks/useTheme'

interface AuthThemeToggleProps {
  /**
   * Stable testid for the host page. Login passes `auth-theme-toggle`; the
   * first-run setup page passes `app-setup-theme-toggle` (the id its existing
   * e2e suite + the testid registry already know).
   */
  'data-testid'?: string
}

/**
 * Light/dark toggle for the unauthenticated screens (login + setup) — the ONE
 * place a signed-out user can flip the theme. Deliberately reuses the app's
 * existing theme mechanism (`useTheme()` → `ConfigClient` → `ThemeProvider`
 * toggles the `dark`/`light` class on <html>); it does NOT hand-roll its own
 * theming. Ported verbatim in behavior from the former page-local
 * `SetupThemeSwitcher` so both screens share one implementation.
 */
export function AuthThemeToggle({
  'data-testid': testId = 'auth-theme-toggle',
}: AuthThemeToggleProps) {
  const { isDarkMode, setTheme } = useTheme()
  return (
    <Button
      variant="ghost"
      size="icon"
      aria-label={isDarkMode ? 'Switch to light theme' : 'Switch to dark theme'}
      data-testid={testId}
      className="absolute right-4 top-4 z-20"
      onClick={() => setTheme(isDarkMode ? 'light' : 'dark')}
    >
      {isDarkMode ? <Sun className="size-5" /> : <Moon className="size-5" />}
    </Button>
  )
}
