/**
 * OnboardingRedirect — owned by the onboarding module.
 *
 * Effect-only component. Mounted inside <BrowserRouter> via the
 * `routerEffects` slot so it can use `useNavigate`/`useLocation`,
 * but renders nothing. Subscribes to auth + completion state and
 * navigates to the first incomplete guide when appropriate.
 *
 * Skip conditions (no redirect):
 *   - User not yet authenticated (auth still bootstrapping).
 *   - DEPLOY-ONLY: ANY user. This deployment hides onboarding from
 *     non-admins entirely (sidebar entry + settings-nav link), so the
 *     forced redirect is disabled for everyone — the admin guard below
 *     plus a non-admin guard beside it. Original admin rationale:
 *   - User is an admin. Admins drive the app — on the desktop shell
 *     they configured providers via Settings directly, and the
 *     phone-over-tunnel surface logs in as that same admin. Forcing
 *     them through `/onboarding` would trap the phone session in a
 *     loop they can't escape (no way to mark guides "done" from
 *     the limited remote surface). Admins can still navigate to
 *     `/onboarding` manually from the sidebar.
 *   - User already on `/onboarding` (don't fight the user).
 *   - User has completed every registered guide.
 *
 * Auth knows nothing about this. Router knows nothing about this.
 * All onboarding-specific logic stays inside the onboarding module.
 */

import { useEffect } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { Stores } from '@ziee/framework/stores'
import type { OnboardingSlot } from './types/OnboardingSlot'

export function OnboardingRedirect() {
  const { isAuthenticated, user, isInitializing } = Stores.Auth
  const { completedGuideIds, loaded } = Stores.Onboarding
  const guides = (Stores.ModuleSystem.slots.get('onboarding') as
    | OnboardingSlot[]
    | undefined) ?? []
  const navigate = useNavigate()
  const location = useLocation()

  useEffect(() => {
    // Match AuthGuard's loading gate: don't redirect while auth is still
    // initializing (e.g. AuthGuard remounts after login form navigation
    // and calls initAuth() which sets isInitializing=true). OnboardingRedirect
    // is rendered OUTSIDE AuthGuard (as a routerEffect sibling of <Routes>),
    // so it must independently respect this guard.
    if (isInitializing) return
    if (!isAuthenticated || !user) return
    // Upstream guard: admins are never force-redirected (they drive the app).
    if (user.is_admin === true) return
    // DEPLOY-ONLY: and neither is anyone else. Onboarding is hidden from
    // non-admins in this deployment (sidebar entry + settings-nav link), so
    // pushing them INTO a guide on every login — one they then have no
    // affordance to navigate back to — is wrong. BOTH lines are load-bearing:
    // together they make the redirect inert deployment-wide. The guide itself
    // stays reachable by an admin opening `/onboarding` deliberately.
    if (!user.is_admin) return
    // Wait until the onboarding store has fetched progress — without this
    // guard a fully-onboarded user would briefly look "incomplete"
    // (empty list, loaded=false) on first paint and get mis-redirected.
    if (!loaded) return
    if (location.pathname.startsWith('/onboarding')) return
    const firstIncomplete = guides.find(g => !completedGuideIds.includes(g.id))
    if (firstIncomplete) {
      navigate(`/onboarding?id=${firstIncomplete.id}`, { replace: true })
    }
  }, [isAuthenticated, user, isInitializing, completedGuideIds, loaded, guides, location.pathname, navigate])

  return null
}
