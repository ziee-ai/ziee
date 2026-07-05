/**
 * Auth fixture — recorded `GET /api/auth/me` for the gallery admin.
 *
 * Provides both a cassette entry (so any real `/me` call replays) and the
 * admin `User` used to seed the Auth store directly at bootstrap, so every
 * permission gate (`hasPermissionNow`) short-circuits on `is_admin` and pages
 * render as an administrator would see them.
 */
import type { MeResponse, User } from '@/api-client/types'
import type { Cassette } from '../mockApi'
import recorded from './recorded/auth.json'

interface AuthFixture {
  me: MeResponse
}

// Typed against the generated response type — a recorded shape that drifts
// from `MeResponse` fails `tsc` here (layer-1 fixture correctness).
const fixture: AuthFixture = recorded as AuthFixture

export const adminMe: MeResponse = fixture.me
export const adminUser: User = fixture.me.user
export const adminPermissions: string[] = fixture.me.permissions

export const authCassette: Cassette = {
  'Auth.me': adminMe,
}
