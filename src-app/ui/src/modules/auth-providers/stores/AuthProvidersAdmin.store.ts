import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  AuthProviderResponse,
  CreateAuthProviderRequest,
  TestProviderResponse,
  UpdateAuthProviderRequest,
} from '@/api-client/types'
import { Stores } from '@/core/stores'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import {
  emitAuthProviderAutoDisabled,
  emitAuthProviderCreated,
  emitAuthProviderDeleted,
  emitAuthProviderUpdated,
} from '@/modules/auth-providers/events'

/**
 * Admin CRUD store for auth providers. Distinct from the public
 * `AuthProviders` store in modules/auth — that one feeds the
 * login-page button row; this one drives the admin settings table.
 *
 * Test results are persisted on the row (`last_test_at`,
 * `last_test_ok`, `last_test_message` — migration 48). The backend
 * enforces "enabled requires a passing probe" on every transition
 * and emits an `auth_provider.auto_disabled` event the store
 * listens to so the Switch snaps back without a manual refresh.
 */
interface AuthProvidersAdminStore {
  providers: AuthProviderResponse[]
  loading: boolean
  saving: boolean
  error: string | null
  /// IDs currently mid-test, so the row's Test button can show a
  /// spinner. Cleared per-id when the call returns.
  testingIds: Set<string>

  __init__: {
    __store__?: () => void
    providers?: () => Promise<void>
  }
  __destroy__?: () => void

  loadProviders: () => Promise<void>
  createProvider: (req: CreateAuthProviderRequest) => Promise<AuthProviderResponse>
  updateProvider: (
    id: string,
    req: UpdateAuthProviderRequest,
  ) => Promise<AuthProviderResponse>
  deleteProvider: (id: string) => Promise<{ affected_user_links: number }>
  /// Test a saved provider. Server persists the result on the row and
  /// auto-disables when the probe fails on an enabled row; we refresh
  /// the list afterwards so the UI shows the new values.
  testProvider: (id: string) => Promise<TestProviderResponse>
  /// Test a config payload WITHOUT saving it to the DB. Used by the
  /// EditDrawer's "Test config" button so admins can verify before
  /// committing.
  testConfig: (req: CreateAuthProviderRequest) => Promise<TestProviderResponse>
}

const GROUP = 'AuthProvidersAdminStore'

export const useAuthProvidersAdminStore = create<AuthProvidersAdminStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      providers: [],
      // Start in the loading state so the first paint shows a spinner, not a
      // spurious "No providers yet" empty state, before __init__ loads.
      // loadProviders always resets this on success/error.
      loading: true,
      saving: false,
      error: null,
      testingIds: new Set<string>(),

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus

          // In-process: created/updated/deleted from local actions. The
          // store doing the mutation also emits, so any other component
          // listening (e.g. the public providers store) can update.
          eventBus.on(
            'auth_provider.created',
            async event => {
              const { provider } = event.data
              set(state => {
                state.providers.push(provider)
                state.providers.sort((a, b) => a.name.localeCompare(b.name))
              })
            },
            GROUP,
          )
          eventBus.on(
            'auth_provider.updated',
            async event => {
              const { provider } = event.data
              set(state => {
                const idx = state.providers.findIndex(p => p.id === provider.id)
                if (idx >= 0) state.providers[idx] = provider
              })
            },
            GROUP,
          )
          eventBus.on(
            'auth_provider.deleted',
            async event => {
              const { providerId } = event.data
              set(state => {
                state.providers = state.providers.filter(
                  p => p.id !== providerId,
                )
                state.testingIds.delete(providerId)
              })
            },
            GROUP,
          )

          // Auto-disable: the backend (or another tab) flipped a row to
          // enabled=false because its probe failed. Reload so the Switch
          // and Alert reflect the canonical state. The toast itself is
          // raised by the action that hit the 400 (or by the manual
          // Test button's caller); the listener's job is just to
          // re-sync.
          eventBus.on(
            'auth_provider.auto_disabled',
            async () => {
              await get().loadProviders()
            },
            GROUP,
          )

          // Cross-device sync: another tab / device mutated a provider.
          // Reload to pick up the new state. `loadProviders` self-guards
          // against in-flight loads.
          const reload = () => void get().loadProviders()
          eventBus.on('sync:auth_provider', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        providers: async () => {
          await get().loadProviders()
        },
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners(GROUP)
      },

      loadProviders: async () => {
        // Self-gate: sync:reconnect fires for every store regardless of
        // audience, so a non-admin must not refetch this admin-only list
        // and trip a 403. Perm must match the endpoint's read gate.
        if (!hasPermissionNow(Permissions.AuthProvidersRead)) {
          // Clear the initial loading state so a non-admin mount doesn't hang
          // on a permanent spinner (loading defaults to true).
          set(s => {
            s.loading = false
          })
          return
        }
        set(s => {
          s.loading = true
          s.error = null
        })
        try {
          const res = await ApiClient.AuthProviders.list(undefined, undefined)
          set(s => {
            s.providers = res
            s.loading = false
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to load providers'
            s.loading = false
          })
        }
      },

      createProvider: async (req: CreateAuthProviderRequest) => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const created = await ApiClient.AuthProviders.create(req, undefined)
          set(s => {
            s.saving = false
          })
          try {
            await emitAuthProviderCreated(created.provider)
          } catch (eventError) {
            console.error(
              'Failed to emit auth provider created event:',
              eventError,
            )
          }
          if (created.connection_warning) {
            try {
              await emitAuthProviderAutoDisabled(
                created.provider.id,
                created.connection_warning,
              )
            } catch (eventError) {
              console.error(
                'Failed to emit auth provider auto_disabled event:',
                eventError,
              )
            }
          }
          return created.provider
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to create provider'
            s.saving = false
          })
          throw e
        }
      },

      updateProvider: async (id, req) => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const updated = await ApiClient.AuthProviders.update(
            { id, ...req },
            undefined,
          )
          set(s => {
            s.saving = false
          })
          try {
            await emitAuthProviderUpdated(updated)
          } catch (eventError) {
            console.error(
              'Failed to emit auth provider updated event:',
              eventError,
            )
          }
          return updated
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to update provider'
            s.saving = false
          })
          // Backend returns 400 AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK
          // when an enable-transition probe fails. Match on the stable
          // error_code (not a substring of the user-facing message and
          // not the presence of `req.enabled` — a duplicate-name or
          // VALIDATION_ERROR 400 on the same PUT would otherwise
          // false-trip the auto_disabled emit). The row has been
          // reverted server-side; the listener reloads the list and
          // the caller (list page / drawer) surfaces its own toast.
          const code = (e as { error_code?: string })?.error_code
          if (code === 'AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK') {
            try {
              await emitAuthProviderAutoDisabled(
                id,
                typeof e?.message === 'string' ? e.message : 'Probe failed',
              )
            } catch (eventError) {
              console.error(
                'Failed to emit auth provider auto_disabled event:',
                eventError,
              )
            }
          }
          throw e
        }
      },

      deleteProvider: async (id: string) => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const res = await ApiClient.AuthProviders.delete({ id }, undefined)
          set(s => {
            s.saving = false
          })
          try {
            await emitAuthProviderDeleted(id)
          } catch (eventError) {
            console.error(
              'Failed to emit auth provider deleted event:',
              eventError,
            )
          }
          return { affected_user_links: res.affected_user_links }
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to delete provider'
            s.saving = false
          })
          throw e
        }
      },

      testProvider: async (id: string) => {
        set(s => {
          s.testingIds.add(id)
        })
        try {
          // Snapshot enabled BEFORE the test; the server may flip it.
          const wasEnabled =
            get().providers.find(p => p.id === id)?.enabled === true
          const res = await ApiClient.AuthProviders.test({ id }, undefined)
          // The originator's SSE filter suppresses the server's own
          // sync_publish (self-echo guard), so the canonical refresh
          // needs an inline reload to surface the new `last_test_*`
          // columns + (possibly) the auto-flipped `enabled` state.
          await get().loadProviders()
          set(s => {
            s.testingIds.delete(id)
          })
          // Additional typed signal for the auto-disable path so a
          // drawer (or other component) listening specifically for
          // auto_disabled can show its own toast / banner. The
          // listener's reload is a no-op when the data already
          // matches the just-loaded state.
          if (wasEnabled && !res.ok) {
            try {
              await emitAuthProviderAutoDisabled(id, res.message)
            } catch (eventError) {
              console.error(
                'Failed to emit auth provider auto_disabled event:',
                eventError,
              )
            }
          }
          return res
        } catch (e: any) {
          set(s => {
            s.testingIds.delete(id)
          })
          return {
            ok: false,
            message: e?.message ?? 'Test failed',
          }
        }
      },

      testConfig: async (req: CreateAuthProviderRequest) => {
        try {
          return await ApiClient.AuthProviders.testConfig(req, undefined)
        } catch (e: any) {
          return {
            ok: false,
            message: e?.message ?? 'Test failed',
          }
        }
      },
    })),
  ),
)
