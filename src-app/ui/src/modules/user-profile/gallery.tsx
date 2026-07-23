/**
 * Dev-gallery seed for the `user-profile` module — the UserProfileWidget in its
 * auth-still-resolving skeleton state, plus the three label states (display
 * name / username fallback / collapsed tooltip). Auto-discovered by the
 * gallery's runtime registry (`@/dev/gallery/support`); never imported by
 * `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdForever, holdPatch, lazyNamed } from '@/dev/gallery/support'
import { adminUser } from '@/dev/gallery/fixtures'

/** Distinct from the cassette fixture's `admin` — see `seedUser`. */
const SEED_USERNAME = 'alovelace'

const widget = () =>
  lazyNamed(
    () => import('@/modules/user-profile/UserProfileWidget'),
    'UserProfileWidget',
  )

/**
 * Seed a SETTLED (non-loading) user so the widget renders its label row.
 *
 * `holdPatch`, not `holdForever`: this is a settled arm, and `holdForever`'s
 * own contract reserves it for LOADING arms that lazy-mount unpredictably. A
 * permanent interval would also re-create the user object every 150ms forever,
 * re-rendering the surface for the life of the page.
 *
 * `AppLayout` IS re-asserted inside the hold, even though it is a persisted
 * store: its rehydration lands asynchronously and clobbers a one-shot
 * `setState`, so the collapsed surface would silently render expanded. The
 * bounded `holdPatch` window is what keeps that from being an unbounded
 * localStorage write loop.
 *
 * `displayName` accepts `null` and `''` on purpose: null is what the wire sends
 * for a cleared display name (the generated `User` type only models the field
 * as optional), and `''` is reachable via the admin create/update path. Both
 * are exactly what the username fallback exists for.
 */
const seedUser =
  (displayName: string | null, collapsed = false) =>
  async () => {
    const { Auth } = await import('@/modules/auth/Auth.store')
    const { AppLayout } = await import(
      '@/modules/layouts/app-layout/AppLayout.store'
    )
    await holdPatch(() => {
      AppLayout.store.setState({ isSidebarCollapsed: collapsed } as any)
      Auth.store.setState({
        // A username DISTINCT from the bootstrap fixture's `admin`. Without
        // this the "no display name" surface is byte-identical to the default
        // seed, so a fallback assertion would pass even if this setup never
        // ran — it would be testing the bootstrap, not the fallback.
        user: { ...adminUser, username: SEED_USERNAME, display_name: displayName },
        isInitializing: false,
        isLoading: false,
      } as any)
    })
  }

export const gallery: ModuleGallery = {
  seeded: [
    // ── UserProfileWidget: auth still resolving (!user && (isInitializing||isLoading)). ─
    {
      slug: 'seeded-s5-user-profile-loading',
      title: 'User profile widget — loading',
      note: '!user && (isInitializing || isLoading) → the skeleton row (UserProfileWidget:86)',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/user-profile/UserProfileWidget'),
        'UserProfileWidget',
      ),
      setup: async () => {
        const { Auth } = await import('@/modules/auth/Auth.store')
        // holdForever (not holdPatch): the widget can mount after a fixed hold
        // window ends under the full pass, so assert on a permanent interval.
        holdForever(() =>
          Auth.store.setState({
            user: null,
            isInitializing: true,
            isLoading: false,
          } as any),
        )
      },
    },

    // ── The sidebar shows the DISPLAY NAME, not the login username. ──────────
    {
      slug: 'seeded-s5-user-profile-display-name',
      title: 'User profile widget — display name',
      note: 'display_name set → the row (and its aria-label/title) name the person, not the login handle',
      path: '/',
      initialPath: '/',
      component: widget(),
      setup: seedUser('Ada Lovelace'),
    },

    // ── …falling back to the username when there is no display name. ────────
    {
      slug: 'seeded-s5-user-profile-no-display-name',
      title: 'User profile widget — username fallback',
      note: 'display_name null → falls back to user.username; must never render a blank row',
      path: '/',
      initialPath: '/',
      component: widget(),
      setup: seedUser(null),
    },

    // ── …and when it is set but BLANK (admin-created/edited rows). This is
    //    the case that distinguishes `||` from `??`: `??` would keep the empty
    //    string and render a nameless row. ─────────────────────────────────
    {
      slug: 'seeded-s5-user-profile-blank-display-name',
      title: 'User profile widget — blank display name',
      note: "display_name '   ' → still falls back to user.username (|| not ??)",
      path: '/',
      initialPath: '/',
      component: widget(),
      setup: seedUser('   '),
    },

    // ── Collapsed sidebar: the same label reaches the hover tooltip. ─────────
    {
      slug: 'seeded-s5-user-profile-collapsed',
      title: 'User profile widget — collapsed (tooltip)',
      note: 'isSidebarCollapsed → the Tooltip carries the same label as the expanded row',
      path: '/',
      initialPath: '/',
      component: widget(),
      setup: seedUser('Ada Lovelace', true),
    },
  ],
}
