// Project knowledge-files store — MIGRATED to the store-kit `defineStore`.
//
// Compare with the old shape: this drops the create()(subscribeWithSelector(
// immer(…))) nesting, the `__init__.__store__` / `__destroy__` scaffolding, the
// `GROUP = 'ProjectFilesStore'` string + its two uses, and the verbose type
// merge. Listeners register via the scoped `on` / `watch` (auto-unsubscribed on
// destroy — no manual removeGroupListeners). Consumers are UNCHANGED: still
// `Stores.ProjectFiles.files`, `Stores.ProjectFiles.loadFiles(...)`, and the new
// `Stores.ProjectFiles.$.currentProjectId` for handler-side reads.

import { defineStore } from '@ziee/framework/store-kit'
import { ApiClient } from '@/api-client'
import type { File as ProjectFile } from '@/api-client/types'
import {
  emitProjectFileAttached,
  emitProjectFileDetached,
} from '@/modules/file/project-extension/events'
import { useProjectDetailStore } from '@/modules/projects/stores'
import { MAX_FILE_UPLOAD_BYTES as MAX_FILE_SIZE } from '@/modules/file/constants'

/**
 * Per-file upload progress. The map key is a synthetic local id (so the same
 * file uploaded twice gets two separate progress rows).
 */
export interface ProjectFileUploadProgress {
  id: string
  filename: string
  size: number
  progress: number
  status: 'pending' | 'uploading' | 'error'
  error?: string
}

export const ProjectFiles = defineStore('ProjectFiles', {
  immer: true,
  state: {
    /** Active project id, mirrored from `ProjectDetail.project.id`. */
    currentProjectId: null as string | null,
    files: [] as ProjectFile[],
    filesLoading: false,
    uploadingFiles: new Map<string, ProjectFileUploadProgress>(),
    selectedFileIds: new Set<string>(),
    attaching: false,
    detaching: false,
    error: null as string | null,
  },

  actions: (set, get) => {
    // Hoisted so sibling actions + init can call it directly (typed), instead of
    // `get().loadFiles` (get() is state-only by design).
    const loadFiles = async (projectId: string) => {
      try {
        set({ filesLoading: true })
        const response = await ApiClient.Project.listFiles({ id: projectId })
        set({ files: response.files, filesLoading: false })
      } catch (error) {
        set({
          error:
            error instanceof Error ? error.message : 'Failed to load project files',
          filesLoading: false,
        })
      }
    }

    return {
      loadFiles,

      attachFile: async (projectId: string, fileId: string) => {
      try {
        set({ attaching: true, error: null })
        await ApiClient.Project.attachFile({ id: projectId, file_id: fileId })
        await emitProjectFileAttached(projectId, fileId)
        set({ attaching: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to attach file',
          attaching: false,
        })
        throw error
      }
    },

    uploadAndAttachFiles: async (projectId: string, files: File[]) => {
      let anySucceeded = false
      await Promise.all(
        files.map(async file => {
          const uploadId = `up_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
          set(state => {
            state.uploadingFiles.set(uploadId, {
              id: uploadId,
              filename: file.name,
              size: file.size,
              progress: 0,
              status: 'pending',
            })
          })

          try {
            if (file.size > MAX_FILE_SIZE) {
              throw new Error(`${file.name} exceeds the per-file size cap`)
            }
            set(state => {
              const entry = state.uploadingFiles.get(uploadId)
              if (entry) entry.status = 'uploading'
            })

            const formData = new FormData()
            formData.append('id', projectId)
            formData.append('file', file)

            const uploaded = await ApiClient.Project.uploadAndAttachFile(
              formData as unknown as { id: string } & FormData,
              {
                fileUploadProgress: {
                  onProgress: progress => {
                    set(state => {
                      const entry = state.uploadingFiles.get(uploadId)
                      if (entry) entry.progress = progress
                    })
                  },
                },
              },
            )

            set(state => {
              state.uploadingFiles.delete(uploadId)
            })
            anySucceeded = true
            await emitProjectFileAttached(projectId, uploaded.id)
          } catch (error) {
            set(state => {
              const entry = state.uploadingFiles.get(uploadId)
              if (entry) {
                entry.status = 'error'
                entry.error =
                  error instanceof Error ? error.message : 'Upload failed'
              }
            })
          }
        }),
      )
      if (anySucceeded) {
        await loadFiles(projectId)
      }
    },

    dismissUploadingFile: (uploadId: string) => {
      set(state => {
        state.uploadingFiles.delete(uploadId)
      })
    },

    deleteFile: async (projectId: string, fileId: string) => {
      try {
        set({ detaching: true, error: null })
        await ApiClient.File.delete({ file_id: fileId })
        await emitProjectFileDetached(projectId, fileId)
        set({ detaching: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to delete file',
          detaching: false,
        })
        throw error
      }
    },

    toggleSelection: (fileId: string) => {
      set(state => {
        if (state.selectedFileIds.has(fileId)) {
          state.selectedFileIds.delete(fileId)
        } else {
          state.selectedFileIds.add(fileId)
        }
      })
    },

    selectAll: () => {
      set(state => {
        for (const file of state.files) state.selectedFileIds.add(file.id)
      })
    },

    deselectAll: () => {
      set(state => {
        state.selectedFileIds.clear()
      })
    },

    batchDelete: async (projectId: string) => {
      const ids = Array.from(get().selectedFileIds)
      if (ids.length === 0) return
      set({ detaching: true, error: null })
      for (const fileId of ids) {
        try {
          await ApiClient.File.delete({ file_id: fileId })
          await emitProjectFileDetached(projectId, fileId)
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : `Failed to delete ${fileId}`,
          })
        }
      }
      set(state => {
        state.detaching = false
        state.selectedFileIds.clear()
      })
    },

      clearError: () => {
        set({ error: null })
      },
    }
  },

  // All listener + cross-store wiring, auto-torn-down on destroy.
  init: ({ on, watch, set, get, actions }) => {
    // Mirror the active project's id and reload files when it changes — atomic
    // reset on project change. Replaces the raw `useProjectDetailStore.subscribe`.
    watch(
      useProjectDetailStore,
      state => state.project?.id ?? null,
      newProjectId => {
        set(state => {
          state.currentProjectId = newProjectId
          state.files = []
          state.uploadingFiles.clear()
          state.selectedFileIds.clear()
        })
        if (newProjectId) void actions.loadFiles(newProjectId)
      },
      { fireImmediately: true },
    )

    // Refresh on attach (e.g. from another tab / sibling emit).
    on('project.file_attached', async event => {
      const current = get().currentProjectId
      if (current && current === event.data.projectId) {
        await actions.loadFiles(current)
      }
    })

    // Local removal on detach — avoids a full reload for a single-row drop.
    on('project.file_detached', async event => {
      const current = get().currentProjectId
      if (current && current === event.data.projectId) {
        set(state => {
          state.files = state.files.filter(f => f.id !== event.data.fileId)
          state.selectedFileIds.delete(event.data.fileId)
        })
      }
    })

    // Clear all state when the project is deleted.
    on('project.deleted', async event => {
      const current = get().currentProjectId
      if (current && current === event.data.projectId) {
        set(state => {
          state.currentProjectId = null
          state.files = []
          state.uploadingFiles.clear()
          state.selectedFileIds.clear()
        })
      }
    })
  },
})
