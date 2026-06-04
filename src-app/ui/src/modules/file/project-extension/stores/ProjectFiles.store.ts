// Project knowledge-files store.
//
// Owns the per-project file list + upload progress + multi-select state.
// Relocated from `modules/projects/stores/ProjectDetail.store.ts` as part
// of the project↔file inversion — the projects module no longer has
// any file state.
//
// Single-project scope: subscribes to `Stores.ProjectDetail.project.id`
// and atomically resets (files cleared, uploads cleared, selection
// cleared) whenever the active project changes. No `Map<projectId, ...>`
// because nothing in the app opens two projects at once.

import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { File as ProjectFile } from '@/api-client/types'
import { Stores } from '@/core/stores'
import {
  emitProjectFileAttached,
  emitProjectFileDetached,
} from '@/modules/file/project-extension/events'
// Import the raw zustand store directly so `.subscribe()` calls don't
// route through the Stores proxy (which would call useEffect+useStore
// hooks on `.__store__` access — see the proxy at core/stores.ts:212).
// This is the file → projects import direction, which is the allowed
// inversion direction post-refactor.
import { useProjectDetailStore } from '@/modules/projects/stores'

/**
 * Per-file upload progress. The map key is a synthetic local id (so the
 * same file uploaded twice gets two separate progress rows). Mirrors
 * the chat composer's FileStore pattern but scoped to the active project.
 */
export interface ProjectFileUploadProgress {
  id: string
  filename: string
  size: number
  progress: number
  status: 'pending' | 'uploading' | 'error'
  error?: string
}

interface ProjectFilesState {
  /** Currently-active project id, mirrored from `Stores.ProjectDetail.project.id`
   *  via the __init__ subscription. Null when no project is open. */
  currentProjectId: string | null

  files: ProjectFile[]
  filesLoading: boolean

  /** In-flight uploads keyed by synthetic local id. Done entries are
   *  removed on success; failed entries remain with `status: 'error'`
   *  until the user clears them. */
  uploadingFiles: Map<string, ProjectFileUploadProgress>

  /** Multi-select state for batch operations (e.g. batch detach).
   *  Cleared atomically on project change. */
  selectedFileIds: Set<string>

  attaching: boolean
  detaching: boolean
  error: string | null

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void

  loadFiles: (projectId: string) => Promise<void>
  attachFile: (projectId: string, fileId: string) => Promise<void>
  /** Upload N files via multipart and attach in one shot. Each gets
   *  its own progress row; errors stay visible until cleared. */
  uploadAndAttachFiles: (projectId: string, files: File[]) => Promise<void>
  dismissUploadingFile: (uploadId: string) => void
  detachFile: (projectId: string, fileId: string) => Promise<void>

  // Selection actions
  toggleSelection: (fileId: string) => void
  selectAll: () => void
  deselectAll: () => void
  batchDetach: (projectId: string) => Promise<void>

  clearError: () => void
}

export const useProjectFilesStore = create<ProjectFilesState>()(
  subscribeWithSelector(
    immer(
      (set, get): ProjectFilesState => ({
        currentProjectId: null,
        files: [],
        filesLoading: false,
        uploadingFiles: new Map(),
        selectedFileIds: new Set(),
        attaching: false,
        detaching: false,
        error: null,

        __init__: {
          __store__: () => {
            const GROUP = 'ProjectFilesStore'
            const eventBus = Stores.EventBus

            // Mirror the active project's id and reload files when it
            // changes. Subscribe directly to the raw zustand store —
            // going through `Stores.ProjectDetail.__store__` triggers
            // useEffect+useStore hooks via the proxy, which corrupts
            // the hook-count on first render (CSS hook-order violation).
            useProjectDetailStore.subscribe(
              state => state.project?.id ?? null,
              newProjectId => {
                // Atomic reset on project change.
                set(state => {
                  state.currentProjectId = newProjectId
                  state.files = []
                  state.uploadingFiles.clear()
                  state.selectedFileIds.clear()
                })
                if (newProjectId) {
                  void get().loadFiles(newProjectId)
                }
              },
              { fireImmediately: true },
            )

            // Refresh on file attach (e.g. from another tab / sibling
            // emit). Re-fetch so the new row's full metadata reaches
            // the list.
            eventBus.on(
              'project.file_attached',
              async event => {
                const current = get().currentProjectId
                if (current && current === event.data.projectId) {
                  await get().loadFiles(current)
                }
              },
              GROUP,
            )

            // Local removal on detach — avoids a full reload for a
            // single-row drop.
            eventBus.on(
              'project.file_detached',
              async event => {
                const current = get().currentProjectId
                if (current && current === event.data.projectId) {
                  set(state => {
                    state.files = state.files.filter(
                      f => f.id !== event.data.fileId,
                    )
                    state.selectedFileIds.delete(event.data.fileId)
                  })
                }
              },
              GROUP,
            )

            // Clear all state when the project is deleted (race against
            // ProjectDetail's own cleanup is benign — both end up empty).
            eventBus.on(
              'project.deleted',
              async event => {
                const current = get().currentProjectId
                if (current && current === event.data.projectId) {
                  set(state => {
                    state.currentProjectId = null
                    state.files = []
                    state.uploadingFiles.clear()
                    state.selectedFileIds.clear()
                  })
                }
              },
              GROUP,
            )
          },
        },

        loadFiles: async projectId => {
          try {
            set({ filesLoading: true })
            const response = await ApiClient.Project.listFiles({
              id: projectId,
            })
            set({ files: response.files, filesLoading: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load project files',
              filesLoading: false,
            })
          }
        },

        attachFile: async (projectId, fileId) => {
          try {
            set({ attaching: true, error: null })
            await ApiClient.Project.attachFile({
              id: projectId,
              file_id: fileId,
            })
            await emitProjectFileAttached(projectId, fileId)
            set({ attaching: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to attach file',
              attaching: false,
            })
            throw error
          }
        },

        uploadAndAttachFiles: async (projectId, files) => {
          let anySucceeded = false
          await Promise.all(
            files.map(async file => {
              const uploadId = `up_${Date.now()}_${Math.random()
                .toString(36)
                .slice(2, 11)}`
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
                      error instanceof Error
                        ? error.message
                        : 'Upload failed'
                  }
                })
              }
            }),
          )
          if (anySucceeded) {
            await get().loadFiles(projectId)
          }
        },

        dismissUploadingFile: uploadId => {
          set(state => {
            state.uploadingFiles.delete(uploadId)
          })
        },

        detachFile: async (projectId, fileId) => {
          try {
            set({ detaching: true, error: null })
            await ApiClient.Project.detachFile({
              id: projectId,
              file_id: fileId,
            })
            await emitProjectFileDetached(projectId, fileId)
            set({ detaching: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to detach file',
              detaching: false,
            })
            throw error
          }
        },

        toggleSelection: fileId => {
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
            for (const file of state.files) {
              state.selectedFileIds.add(file.id)
            }
          })
        },

        deselectAll: () => {
          set(state => {
            state.selectedFileIds.clear()
          })
        },

        batchDetach: async projectId => {
          const ids = Array.from(get().selectedFileIds)
          if (ids.length === 0) return
          set({ detaching: true, error: null })
          // Detach sequentially to keep event order predictable; the
          // backend doesn't expose a batch endpoint and per-row detaches
          // are cheap (single-row DELETE).
          for (const fileId of ids) {
            try {
              await ApiClient.Project.detachFile({
                id: projectId,
                file_id: fileId,
              })
              await emitProjectFileDetached(projectId, fileId)
            } catch (error) {
              set({
                error:
                  error instanceof Error
                    ? error.message
                    : `Failed to detach ${fileId}`,
              })
              // Continue with remaining detaches so a single failure
              // doesn't strand the user; the error message surfaces
              // the failed one.
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

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('ProjectFilesStore')
        },
      }),
    ),
  ),
)
