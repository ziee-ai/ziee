import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { hardwareState, type HardwareState } from './state'
import { resetSseGuards } from './actions/_sseConnect'
import type { Actions } from './actions.gen'

const HardwareDef = defineStore<HardwareState, Actions>('Hardware', {
  immer: true,
  state: hardwareState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions, onCleanup }) => {
    // `/api/hardware/info` requires hardware::read (not user-held). A user
    // reaching /hardware-monitor via hardware::monitor alone would otherwise
    // 403 on this eager fetch at store-mount. SSE connect is gated by the
    // route perm separately.
    if (hasPermissionNow(Permissions.HardwareRead)) {
      void actions.loadHardwareInfo()
    }
    // Abort the module-scope SSE controller on store destroy so it doesn't
    // outlive the store (the proxy may destroy it after a 5s grace period; an
    // in-flight SSE keeps running otherwise and blocks reconnect). (audit 09 B-8)
    onCleanup(() => {
      resetSseGuards()
    })
  },
})

export const Hardware = registerLazyStore(HardwareDef)
export const useHardwareStore = HardwareDef.store

// Raw store for direct access (gallery seed, type augmentation, etc.).
export { HardwareDef }
