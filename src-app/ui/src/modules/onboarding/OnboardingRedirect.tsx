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
import { Stores } from '@/core/stores'
import type { OnboardingSlot } from './types/OnboardingSlot'

export function OnboardingRedirect() {
  const { isAuthenticated, user } = Stores.Auth
  const guides = (Stores.ModuleSystem.slots.get('onboarding') as
    | OnboardingSlot[]
    | undefined) ?? []
  const navigate = useNavigate()
  const location = useLocation()

  useEffect(() => {
    if (!isAuthenticated || !user) return
    // Defensive: a partial user payload (e.g. brief incomplete state
    // during refresh) must not trigger a redirect. Treat anything but
    // an explicit `true` as "not an admin → keep evaluating" but only
    // when the completion list is actually an array.
    if (user.is_admin === true) return
    const completed = Array.isArray(user.completed_onboarding_ids)
      ? user.completed_onboarding_ids
      : null
    if (completed === null) return
    if (location.pathname.startsWith('/onboarding')) return
    const firstIncomplete = guides.find(g => !completed.includes(g.id))
    if (firstIncomplete) {
      navigate(`/onboarding?id=${firstIncomplete.id}`, { replace: true })
    }
  }, [isAuthenticated, user, guides, location.pathname, navigate])

  return null
}
