/**
 * Desktop Tunnel Auth module.
 *
 * Owns the PHONE-FACING authentication surface for the desktop app's
 * remote-access tunnel: magic-link consumption + password-only login.
 * Registered ONLY in the desktop bundle — the backend serves the same
 * desktop bundle to phones over the ngrok tunnel, and the AuthGuard
 * override branches on `isTauriView` to decide what to render when
 * the visitor isn't authenticated.
 *
 * This module's only public route is `/auth/magic/:token`, marked
 * `requiresAuth: false` so the AuthGuard doesn't intercept it
 * before the exchange can run.
 *
 * The `PhoneAuthPage` (password form / "open desktop for magic
 * link" message) is NOT a route — it's rendered directly by
 * `desktop/ui/src/modules/auth/AuthGuard.tsx` when there's no Tauri
 * webview and the user isn't authenticated. That keeps the URL
 * shape clean (`/` stays `/` on the phone).
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { lazy } from 'react'

const MagicLinkPage = lazy(() =>
  import('./MagicLinkPage').then(m => ({ default: m.MagicLinkPage })),
)

const tunnelAuthModule: AppModule = createModule({
  metadata: {
    name: 'tunnel-auth-desktop',
    version: '1.0.0',
    description:
      'Phone-facing auth surface for the desktop app served over the ngrok tunnel.',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/auth/magic/:token',
      element: MagicLinkPage,
      requiresAuth: false,
    },
    // Bare /auth/magic (no token in URL) — the page handles the
    // missing-param case by rendering a "Missing token" Result with
    // guidance to grab a fresh QR from the desktop app.
    {
      path: '/auth/magic',
      element: MagicLinkPage,
      requiresAuth: false,
    },
  ],
})

export default tunnelAuthModule
