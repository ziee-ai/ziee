import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { runtimeVersionState, type RuntimeVersionState } from './state'
import type { Actions } from './actions.gen'

const RuntimeVersionDef = defineStore<RuntimeVersionState, Actions>('RuntimeVersion', {
  state: runtimeVersionState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions }) => {
    on('runtime_version.created', event => {
      const state = get()
      if (!state.versions.find(v => v.id === event.data.version.id)) {
        set(s => {
          s.versions = [...s.versions, event.data.version]
        })
      }
    })
    on('runtime_version.deleted', event => {
      set(s => {
        s.versions = s.versions.filter(v => v.id !== event.data.versionId)
      })
    })
    on('runtime_version.default_changed', event => {
      set(s => {
        const version = s.versions.find(v => v.id === event.data.versionId)
        if (!version) return
        s.versions = s.versions.map(v => ({
          ...v,
          is_system_default:
            v.engine === version.engine ? v.id === event.data.versionId : v.is_system_default,
        }))
      })
    })
    // Cross-device sync: reload on a remote download/delete/default change, or
    // after an SSE reconnect. loadVersions self-gates on RuntimeVersionRead.
    const reload = () => void actions.loadVersions()
    on('sync:runtime_version', reload)
    on('sync:reconnect', reload)
    void actions.loadVersions()
  },
})

export const RuntimeVersion = registerLazyStore(RuntimeVersionDef)
export const useRuntimeVersionStore = RuntimeVersionDef.store
