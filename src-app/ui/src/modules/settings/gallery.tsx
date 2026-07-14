/**
 * Dev-gallery seed for the `settings` module — a SHADOW of the `/settings`
 * landing that renders the real nav shell (the enumerated route's element is
 * `() => null`). Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyNamed } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  seeded: [
    // ── SHADOW: /settings landing. The enumerated `/settings` route's element is
    // `() => null` — its real content (the settings nav + a redirect to the first
    // section) lives in `SettingsLayoutDef`, which the page grid doesn't apply, so
    // the enumerated surface is blank. Render `SettingsPage` (the nav shell) inside
    // its own router landed on a section so the genuine settings landing chrome is
    // reviewable. ────────────────────────────────────────────────────────────────
    {
      slug: 'settings',
      title: 'Settings landing (nav shell)',
      note: '/settings redirects to the first section via SettingsLayout; the page grid renders the null index element. This renders SettingsPage on the first section so the real settings nav chrome is reviewable.',
      // Mount SettingsPage under the frame's OWN router (no nested MemoryRouter —
      // React Router forbids that) at a concrete section so its redirect effect is
      // a no-op and the nav menu + header render. The section Outlet has no child
      // route (each section is reviewed as its own enumerated surface), so the
      // content area is intentionally empty — the point is the nav shell.
      path: '/settings/:section',
      initialPath: '/settings/general',
      component: lazyNamed(
        () => import('@/modules/settings/SettingsPage'),
        'default',
      ),
    },
  ],
}
