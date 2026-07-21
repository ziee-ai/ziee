import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { fileVersionsState } from './state'

const FileVersionsDef = defineStore('FileVersions', {
  immer: true,
  state: fileVersionsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions }) => {
    // Drop cached version TEXT (incl. error sentinels) so a reopened/old-version
    // view refetches fresh. Cheap (small cache).
    const dropVersionText = () =>
      set(s => {
        s.versionTextCache = new Map()
        s.versionTextLoadingSet = new Set()
      })
    // A specific file changed (restore / MCP edit / sandbox version-back). Only
    // REFETCH that file's version list — not every tracked file's.
    on('sync:file', (event: { data?: { id?: string } }) => {
      const fileId = event?.data?.id
      dropVersionText()
      if (fileId && get().versionsByFile.has(fileId)) {
        void actions.loadVersions(fileId)
      } else if (!fileId) {
        // Defensive: a sync:file with no id → reload all tracked.
        Array.from(get().versionsByFile.keys()).forEach(fid => void actions.loadVersions(fid))
      }
    })
    // Reconnect: we may have missed events → reload EVERY tracked file.
    on('sync:reconnect', () => {
      dropVersionText()
      Array.from(get().versionsByFile.keys()).forEach(fid => void actions.loadVersions(fid))
    })
  },
})
export const FileVersions = registerLazyStore(FileVersionsDef)
export const useFileVersionsStore = FileVersionsDef.store
