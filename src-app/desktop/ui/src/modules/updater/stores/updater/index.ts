import { getVersion } from '@tauri-apps/api/app'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { updaterState } from './state'

const UpdaterDef = defineStore('Updater', {
  immer: true,
  state: updaterState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ set, actions, onCleanup }) => {
    // Record the running version, then silently check for an update so the
    // sidebar card can surface one; keep checking once a day while open.
    void (async () => {
      try {
        const v = await getVersion()
        set(s => { s.currentVersion = v })
      } catch {
        // Non-Tauri context (e.g. unit test) — leave version null.
      }
      await actions.check()
      actions.startDailyChecks()
    })()
    onCleanup(() => {
      actions.stopPolling()
      actions.stopDailyChecks()
    })
  },
})

export const Updater = registerLazyStore(UpdaterDef)
export const useUpdaterStore = UpdaterDef.store

export type { UpdaterState, UpdaterSet, UpdaterGet } from './state'
